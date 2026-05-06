"""
Dave STAGE-protocol QLoRA fine-tune on Qwen3.5-4B + GGUF export.

CRITICAL DAVE-SPECIFIC SETTINGS:
  - enable_thinking=True at apply_chat_template. Dave's training data has
    <think>...</think>...reply structure in every assistant turn (Two-Is
    architecture: reasoning layer + output layer both in Dave's voice).
    With enable_thinking=False, Qwen's chat template can strip/modify those
    blocks during formatting, and the model never learns to emit them. With
    enable_thinking=True, the <think> blocks pass through verbatim and the
    model learns to produce them at inference, which is what LM Studio's
    "Reasoning" section requires to display thinking traces.
  - save_total_limit=10 so all epoch checkpoints persist.

GGUF EXPORT (failsafe):
  - Runs after training. Adapter is saved to disk first; GGUF is a
    derivative artifact. If GGUF fails, adapter is preserved and you
    rerun with --skip-train to retry GGUF only.
  - Default quantization q4_k_m (LM Studio sweet spot, ~2.5 GB for 4B).
  - Other options: q5_k_m, q8_0, f16. Pass via --gguf-quant.

USAGE:
    # Full training + GGUF
    python finetune_dave.py --data ./dave_train.jsonl --output ./dave_adapter

    # Training only, no GGUF
    python finetune_dave.py --data ./dave_train.jsonl --output ./dave_adapter --no-gguf

    # GGUF only, from existing adapter (recovery path)
    python finetune_dave.py --skip-train --output ./dave_adapter

The GGUF lands at ./dave_gguf/ by default. Copy that file into LM Studio's
models directory to load it.

GGUF conversion takes 5-10 min on first run (Unsloth compiles llama.cpp
binaries). Subsequent runs are faster.
"""
import argparse
import os
import sys

# Unsloth MUST be imported before transformers/peft/trl.
from unsloth import FastModel
from datasets import load_dataset
from trl import SFTTrainer, SFTConfig
from transformers import DataCollatorForSeq2Seq


def do_train(args, model, tokenizer):
    """Attach LoRA, load data, train, save adapter."""
    model = FastModel.get_peft_model(
        model,
        r=args.rank,
        target_modules=[
            "q_proj", "k_proj", "v_proj", "o_proj",
            "gate_proj", "up_proj", "down_proj",
        ],
        lora_alpha=args.rank,
        lora_dropout=0,
        bias="none",
        use_gradient_checkpointing="unsloth",
        random_state=3407,
    )

    print(f"[data] {args.data}")
    ds = load_dataset("json", data_files=args.data, split="train")
    print(f"[data] {len(ds)} samples")

    def fmt(examples):
        out = []
        for msgs in examples["messages"]:
            text = tokenizer.apply_chat_template(
                msgs,
                tokenize=False,
                add_generation_prompt=False,
                enable_thinking=True,         # Two-Is: <think> blocks must survive
            )
            out.append(text)
        return {"text": out}

    ds = ds.map(fmt, batched=True, remove_columns=ds.column_names)

    print("\n[sample] first formatted text:\n" + "-" * 60)
    print(ds[0]["text"][:800])
    print("-" * 60 + "\n")

    # Sanity-check that <think> blocks survived chat-template formatting.
    # If they didn't, the model won't learn to emit them at inference and
    # LM Studio will show no reasoning section even with thinking-on UI.
    with_think = sum(1 for r in ds if "<think>" in r["text"])
    print(f"[sanity] {with_think}/{len(ds)} samples contain <think> in formatted text")
    if with_think < len(ds) * 0.95:
        print(f"[WARN] <think> blocks are missing from most samples after chat-template")
        print(f"[WARN] formatting. The model will not learn to emit thinking.")
        print(f"[WARN] check that enable_thinking=True and the model's chat_template")
        print(f"[WARN] doesn't strip thinking from training-time messages.")
        sys.exit(1)

    sft_config = SFTConfig(
        output_dir=args.output,
        per_device_train_batch_size=args.batch,
        gradient_accumulation_steps=args.grad_accum,
        warmup_ratio=0.05,
        num_train_epochs=args.epochs,
        learning_rate=args.lr,
        logging_steps=5,
        save_strategy="epoch",
        save_total_limit=10,
        optim="adamw_8bit",
        weight_decay=0.01,
        lr_scheduler_type="cosine",
        seed=3407,
        report_to="none",
        max_seq_length=args.max_seq,
        dataset_text_field="text",
        dataset_num_proc=1,
        packing=False,
        bf16=True,
    )

    trainer = SFTTrainer(
        model=model,
        tokenizer=tokenizer,
        train_dataset=ds,
        args=sft_config,
        data_collator=DataCollatorForSeq2Seq(tokenizer=tokenizer),
    )

    print("[train] starting")
    trainer.train()

    print(f"[save] {args.output}")
    model.save_pretrained(args.output)
    tokenizer.save_pretrained(args.output)
    print("[saved] adapter persisted to disk")
    return model


def do_gguf(args, model, tokenizer):
    """Export merged model as GGUF. Failure here does not destroy the adapter."""
    print(f"\n[gguf] exporting to {args.gguf_output} (quant: {args.gguf_quant})")
    print("[gguf] this takes 5-10 min on first run (compiling llama.cpp)")
    try:
        model.save_pretrained_gguf(
            args.gguf_output,
            tokenizer,
            quantization_method=args.gguf_quant,
        )
        # Locate the produced file
        gguf_files = []
        for root, _, files in os.walk(args.gguf_output):
            for fn in files:
                if fn.endswith(".gguf"):
                    gguf_files.append(os.path.join(root, fn))
        print(f"[gguf] success")
        if gguf_files:
            for gf in gguf_files:
                size_mb = os.path.getsize(gf) / (1024 * 1024)
                print(f"[gguf] {gf}  ({size_mb:.0f} MB)")
        print(f"[gguf] for LM Studio: copy the .gguf into its models folder")
        print(f"       (typically C:\\Users\\<you>\\.cache\\lm-studio\\models\\dave\\dave\\)")
        return True
    except Exception as e:
        print(f"\n[gguf] FAILED: {type(e).__name__}: {e}")
        print(f"[gguf] adapter is still safe at: {args.output}")
        print(f"[gguf] to retry just the GGUF step:")
        print(f"       python finetune_dave.py --skip-train --output {args.output}")
        print(f"[gguf] alternative quantizations to try: q5_k_m, q8_0, f16")
        return False


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--data", default="./dave_train.jsonl")
    p.add_argument("--output", default="./dave_adapter")
    p.add_argument("--model", default="unsloth/Qwen3.5-4B")
    p.add_argument("--max_seq", type=int, default=2048)
    p.add_argument("--epochs", type=int, default=4)
    p.add_argument("--lr", type=float, default=2e-4)
    p.add_argument("--rank", type=int, default=32)
    p.add_argument("--batch", type=int, default=1)
    p.add_argument("--grad_accum", type=int, default=4)
    # GGUF / failsafe controls
    p.add_argument("--skip-train", action="store_true",
                   help="Skip training; load existing adapter and only do GGUF export.")
    p.add_argument("--no-gguf", action="store_true",
                   help="Skip GGUF export at the end.")
    p.add_argument("--gguf-quant", default="q4_k_m",
                   help="GGUF quantization (default: q4_k_m). "
                        "Alternatives: q5_k_m, q8_0, f16.")
    p.add_argument("--gguf-output", default="./dave_gguf",
                   help="GGUF output directory (default: ./dave_gguf)")
    args = p.parse_args()

    # =========================================================================
    # Load model
    # =========================================================================
    if args.skip_train:
        if not os.path.isdir(args.output):
            print(f"[error] --skip-train set but adapter dir not found: {args.output}")
            sys.exit(1)
        print(f"[load] adapter at {args.output} (skip-train mode)")
        model, tokenizer = FastModel.from_pretrained(
            model_name=args.output,
            max_seq_length=args.max_seq,
            load_in_4bit=True,
            full_finetuning=False,
        )
    else:
        print(f"[load] {args.model}")
        model, tokenizer = FastModel.from_pretrained(
            model_name=args.model,
            max_seq_length=args.max_seq,
            load_in_4bit=True,
            full_finetuning=False,
        )
        model = do_train(args, model, tokenizer)

    # =========================================================================
    # GGUF export (or skip)
    # =========================================================================
    if args.no_gguf:
        print("[done] (--no-gguf set; skipping GGUF export)")
        return

    success = do_gguf(args, model, tokenizer)
    if success:
        print("[done]")
    else:
        # Non-zero exit so any wrapping script knows GGUF failed,
        # but the adapter is preserved.
        sys.exit(2)


if __name__ == "__main__":
    main()
