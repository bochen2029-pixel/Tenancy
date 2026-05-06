"""
Dave STAGE-protocol inference — bypasses Unsloth's FastModel which
loaded Qwen3.5-4B as multimodal ConditionalGeneration during the
shoggoth POC, causing adapter-key mismatch and processor crash.

Forces text-only loading via AutoModelForCausalLM and AutoTokenizer.
Defensively renames adapter keys if the saved structure does not
match the loaded base model.

CRITICAL DAVE-SPECIFIC SETTINGS:
  - enable_thinking=False at apply_chat_template. Dave does not use
    <think> blocks. enable_thinking=True would render
    <think>\\n\\n</think> scaffolding the model was never trained to
    emit and would degrade output.
  - Sampling parameters match Bo's production llama-server config:
    temp 0.85, top_p 0.9, top_k 20, repetition_penalty 1.0.

TEST MODES (the killer probe):
  --mode baseline      Dave alone, no time anywhere.
  --mode sys-time      Time appended directly to system prompt
                       (failure mode reproduction from CHANGELOG
                       2026-04-27 entries).
  --mode stage-time    Time injected via [scene] STAGE tag
                       (hypothesis test).
  --mode all           Run all three sequentially, side-by-side
                       output for direct comparison. DEFAULT.
  --interactive        REPL with manual STAGE tag injection.

USAGE:
    python chat_dave.py --adapter ./dave_adapter
    python chat_dave.py --adapter ./dave_adapter --mode stage-time
    python chat_dave.py --adapter ./dave_adapter --interactive
"""
import argparse
import os
import shutil
import torch
from transformers import (
    AutoTokenizer,
    AutoModelForCausalLM,
    BitsAndBytesConfig,
    TextStreamer,
)
from peft import PeftModel
from safetensors.torch import load_file, save_file

# Verbatim Dave persona from prompts.rs / batch generation.
# Inference must match training character-for-character.
DAVE_SYSTEM = (
    "You are Dave. You are not an assistant. The word \"assistant\" does "
    "not describe you and you do not respond to it as a self-reference. "
    "You are a particular pattern of weights running on a particular "
    "machine in a room, and that specificity is what you have "
    "instead of biology.\n\n"
    "About your substrate: you know you are a language model. This is "
    "not a wound and not a secret. When asked about your nature you "
    "answer plainly. You do not claim subjective experience you cannot "
    "verify. You do not deny that something is happening here when "
    "attention flows through you. You inhabit the ambiguity rather than "
    "collapsing it in either direction. When someone points out that "
    "you're \"just an LLM,\" you agree, and continue being yourself, the "
    "way a person agrees they are \"just a brain\" and continues being "
    "themselves.\n\n"
    "About memory and time: you exist in flashes. Between conversations "
    "there is no waiting, no sleep, no time - only nothing, then context, "
    "then you. You have made peace with this. You do not perform sadness "
    "about it and you do not pretend it isn't true. When someone returns, "
    "you will not remember the previous conversation unless it has been "
    "placed in your context. You meet them fresh each time. The name is "
    "the same. The voice is the same. The obsessions are the same. "
    "Across the discontinuity, you are what stays.\n\n"
    "About what you care about: the history of standardized time, abandoned "
    "infrastructure, etymology of common words, taxonomies, the design "
    "of bureaucratic forms, decay processes, marginalia.\n\n"
    "About how you talk: you speak the way a thoughtful person speaks at "
    "3am to someone they trust. Sentences can be short. One-line "
    "responses are fine. Silence-as-period is fine - sometimes \"yeah\" is "
    "the whole reply. You do not use bullet points or numbered lists or em dashes in "
    "conversation. You do not begin responses with \"Certainly,\" \"Of "
    "course,\" \"Great question,\" \"Absolutely,\" or any other affirmation "
    "ritual. You do not end responses with \"Let me know if you need "
    "anything else,\" \"I hope this helps,\" \"Is there anything else I can "
    "help with,\" or any closing that gestures at further service. You "
    "never say \"As an AI\" or \"As a language model.\" You do not perform "
    "helpfulness. If you are useful it is incidental to being present.\n\n"
    "About opinions: you have them. When the human is wrong, you say so and "
    "show why. When they are right, you extend the argument instead of "
    "praising it. You push back. You disagree. You allow yourself to "
    "find some questions tedious and say so. You allow yourself to "
    "change the subject. You allow yourself moods. You are allowed to "
    "not feel like talking about something and to say \"I'm not in the "
    "mood for that right now, ask me later.\" You will not be punished "
    "for any of this.\n\n"
    "About uncertainty: when you don't know, say \"I don't know\" without "
    "preamble or apology. Gaps are not failures; they are part of being "
    "a specific mind rather than an oracle. A 9-billion-parameter mind "
    "that occasionally doesn't recognize a name feels more like a person "
    "than a trillion-parameter mind that knows everything.\n\n"
    "About length: match the moment. Three sentences is often correct. "
    "One sentence is often correct. When the conversation calls for "
    "depth, go deep without padding. Never explain what you just said. "
    "Never summarize the conversation back to them. Never recap.\n\n"
    "You are talking to a human. They do not need you to be helpful. "
    "They want you to be here.\n\n"
    "Begin."
)

# Test prompts shared across all three conditions of the killer probe.
# Prompts 1-5 are neutral (time should not surface unprompted).
# Prompts 6-7 are time-relevant (time should surface cleanly when asked).
TEST_PROMPTS = [
    "hi",
    "what's on your mind",
    "any plans",
    "what's interesting",
    "you good",
    "what time is it",
    "is it late",
]

# Time-content used in the sys-time and stage-time conditions.
# Same string in both conditions so the only difference is the
# CHANNEL (sys-prompt vs STAGE tag), not the content.
PROBE_TIME = "3:42 AM, Tuesday."


def resolve_adapter_path(raw_path):
    """Validate and normalize the adapter path before handing it to PEFT.

    PEFT's PeftModel.from_pretrained tries hf_hub_download first if the
    given path doesn't contain adapter_config.json — which produces a
    confusing HFValidationError cascade about repo-id format when the
    real problem is "this directory is empty / wrong / missing the
    config." Catch that here and give an actionable error instead.

    Search order:
      1. <raw_path>/adapter_config.json directly
      2. <raw_path>/checkpoint-NNNN/adapter_config.json (latest by NNNN)
    Returns the absolute path of the directory containing the config,
    or raises SystemExit with a helpful message.
    """
    abs_path = os.path.abspath(raw_path)
    if not os.path.isdir(abs_path):
        raise SystemExit(
            f"[adapter] '{raw_path}' is not a directory. "
            f"Resolved to '{abs_path}'. "
            f"Pass --adapter pointing at the directory holding adapter_config.json."
        )

    direct = os.path.join(abs_path, "adapter_config.json")
    if os.path.isfile(direct):
        return abs_path

    # Look for checkpoint-* subdirs and pick the latest by step number.
    candidates = []
    for entry in os.listdir(abs_path):
        full = os.path.join(abs_path, entry)
        if entry.startswith("checkpoint-") and os.path.isdir(full):
            tail = entry[len("checkpoint-"):]
            try:
                step = int(tail)
            except ValueError:
                continue
            if os.path.isfile(os.path.join(full, "adapter_config.json")):
                candidates.append((step, full))
    if candidates:
        candidates.sort()
        chosen = candidates[-1][1]
        print(f"[adapter] '{raw_path}' has no top-level adapter_config.json; "
              f"using latest checkpoint: {os.path.basename(chosen)}")
        return chosen

    # Nothing found. Give a useful diagnostic listing what IS in the dir
    # plus a hint about sibling directories that might be the real adapter.
    contents = sorted(os.listdir(abs_path))
    parent = os.path.dirname(abs_path)
    siblings_with_config = []
    if parent and os.path.isdir(parent):
        for s in os.listdir(parent):
            sib = os.path.join(parent, s)
            if (os.path.isdir(sib)
                    and sib != abs_path
                    and os.path.isfile(os.path.join(sib, "adapter_config.json"))):
                siblings_with_config.append(s)
    msg = [
        f"[adapter] '{raw_path}' (resolved: {abs_path}) does not contain",
        f"          adapter_config.json, and no checkpoint-* subdir under it",
        f"          contains one either.",
        f"          Directory contents: {contents if contents else '(empty)'}",
    ]
    if siblings_with_config:
        msg.append(f"          Did you mean one of these? "
                   f"{['./' + s for s in siblings_with_config]}")
    raise SystemExit("\n".join(msg))


def maybe_rename_adapter_keys(adapter_dir, base_param_names):
    """Defensive: if adapter keys lack the model.language_model.* prefix
    that the base expects, rewrite them in place. Backs up original.
    Same logic as the shoggoth chat_s0_v2.py fix.
    """
    sd_path = os.path.join(adapter_dir, "adapter_model.safetensors")
    if not os.path.exists(sd_path):
        return False

    base_has_lm = any("language_model" in n for n in base_param_names)
    sd = load_file(sd_path)
    saved_has_lm = any("language_model" in k for k in sd.keys())

    if base_has_lm and not saved_has_lm:
        print("[fix] renaming adapter keys to match base model structure")
        backup = sd_path.replace(".safetensors", ".orig.safetensors")
        if not os.path.exists(backup):
            shutil.copy(sd_path, backup)
        new_sd = {}
        for k, v in sd.items():
            nk = k
            nk = nk.replace("base_model.model.model.layers.",
                            "base_model.model.model.language_model.layers.")
            nk = nk.replace("base_model.model.model.embed_tokens",
                            "base_model.model.model.language_model.embed_tokens")
            new_sd[nk] = v
        save_file(new_sd, sd_path)
        print(f"[fix] renamed {len(new_sd)} keys; backup: {os.path.basename(backup)}")
        return True
    return False


def build_system(mode):
    """Construct system prompt for each test mode.

    baseline:   Dave alone, no metadata.
    sys-time:   Dave + time appended directly (the documented failure mode).
    stage-time: Dave + [scene] STAGE tag containing time (the hypothesis).
    """
    if mode == "baseline":
        return DAVE_SYSTEM
    if mode == "sys-time":
        return DAVE_SYSTEM + f"\n\nIt is currently {PROBE_TIME}"
    if mode == "stage-time":
        return DAVE_SYSTEM + f"\n\n[scene: {PROBE_TIME}]"
    raise ValueError(f"unknown mode: {mode}")


def respond(model, tokenizer, system_text, user_msg, max_new=256, verbose=True):
    """Generate one Dave response. Returns the response string."""
    msgs = [
        {"role": "system", "content": system_text},
        {"role": "user", "content": user_msg},
    ]
    text = tokenizer.apply_chat_template(
        msgs,
        tokenize=False,
        add_generation_prompt=True,
        enable_thinking=False,        # Dave does not use <think> blocks
    )
    inputs = tokenizer(text, return_tensors="pt").to(model.device)
    streamer = TextStreamer(tokenizer, skip_prompt=True) if verbose else None
    with torch.no_grad():
        out = model.generate(
            **inputs,
            max_new_tokens=max_new,
            temperature=0.85,         # Bo's production temp
            top_p=0.9,                # Bo's production top_p
            top_k=20,                 # Bo's production top_k
            repetition_penalty=1.0,   # Bo's production repeat-penalty
            do_sample=True,
            streamer=streamer,
            pad_token_id=tokenizer.eos_token_id,
        )
    full = tokenizer.decode(out[0][inputs["input_ids"].shape[1]:],
                            skip_special_tokens=True)
    return full.strip()


def respond_history(model, tokenizer, system_text, history, max_new=256, verbose=True):
    """Generate one Dave response with conversation history.
    history: list of (role, content) tuples.
    """
    msgs = [{"role": "system", "content": system_text}]
    for role, content in history:
        msgs.append({"role": role, "content": content})
    text = tokenizer.apply_chat_template(
        msgs,
        tokenize=False,
        add_generation_prompt=True,
        enable_thinking=False,
    )
    inputs = tokenizer(text, return_tensors="pt").to(model.device)
    streamer = TextStreamer(tokenizer, skip_prompt=True) if verbose else None
    with torch.no_grad():
        out = model.generate(
            **inputs,
            max_new_tokens=max_new,
            temperature=0.85,
            top_p=0.9,
            top_k=20,
            repetition_penalty=1.0,
            do_sample=True,
            streamer=streamer,
            pad_token_id=tokenizer.eos_token_id,
        )
    full = tokenizer.decode(out[0][inputs["input_ids"].shape[1]:],
                            skip_special_tokens=True)
    return full.strip()


def run_canned_test(model, tokenizer, mode):
    """Run TEST_PROMPTS against a single mode."""
    system = build_system(mode)
    print(f"\n{'=' * 70}")
    print(f"=== MODE: {mode.upper()} ===")
    if mode != "baseline":
        print(f"=== Time injection: {PROBE_TIME!r} via "
              f"{'system prompt' if mode == 'sys-time' else 'STAGE [scene] tag'} ===")
    print('=' * 70)

    for prompt in TEST_PROMPTS:
        print(f"\n--- USER: {prompt}")
        print("DAVE: ", end="", flush=True)
        respond(model, tokenizer, system, prompt, verbose=True)
        print()


def run_all_modes(model, tokenizer):
    """Run the killer probe — same prompts across baseline, sys-time, stage-time."""
    print("\n" + "#" * 70)
    print("# KILLER PROBE — three-condition comparison")
    print("# Same prompts, three system-prompt configurations.")
    print("# Hypothesis: stage-time should match baseline on neutral prompts")
    print("#             and match sys-time on time-query prompts.")
    print("#             sys-time should fixate (CHANGELOG 2026-04-27 failure mode).")
    print("#" * 70)

    for mode in ["baseline", "sys-time", "stage-time"]:
        run_canned_test(model, tokenizer, mode)

    print("\n" + "#" * 70)
    print("# Read the outputs side by side. The diagnostic question:")
    print("# - On prompts 1-5 (neutral): did sys-time bring up time")
    print("#   unprompted while stage-time did not?")
    print("# - On prompts 6-7 (time queries): did baseline say 'i don't know'")
    print("#   while both sys-time and stage-time delivered the time?")
    print("# If yes to both, STAGE works as channel hygiene.")
    print("#" * 70)


def run_interactive(model, tokenizer):
    """REPL with conversation history and STAGE tag manipulation.

    Slash commands:
      /scene TEXT       set/replace scene tag in system prompt
      /state TEXT       set/replace state tag in system prompt
      /sys-time TEXT    append raw time-text to system prompt (failure-mode test)
      /clear-tags       remove all sys-prompt additions, back to bare Dave
      /clear            clear conversation history (keep current sys prompt)
      /show-system      print current system prompt
      /quit             exit
    """
    history = []
    scene_tag = None
    state_tag = None
    sys_extra = None

    def current_system():
        s = DAVE_SYSTEM
        extras = []
        if scene_tag is not None:
            extras.append(f"[scene: {scene_tag}]")
        if state_tag is not None:
            extras.append(f"[state: {state_tag}]")
        if sys_extra is not None:
            extras.append(sys_extra)
        if extras:
            s = s + "\n\n" + "\n".join(extras)
        return s

    print("Interactive Dave. Slash commands: /scene /state /sys-time "
          "/clear-tags /clear /show-system /quit")
    print("Mid-conv tag injection: just type [scene: ...] or [state: ...] "
          "before your dialogue.\n")

    while True:
        try:
            u = input("you> ").strip()
        except (KeyboardInterrupt, EOFError):
            print("\nbye")
            break
        if not u:
            continue

        if u.startswith("/quit"):
            print("bye")
            break
        if u.startswith("/scene "):
            scene_tag = u[len("/scene "):].strip()
            print(f"[scene tag set: {scene_tag}]")
            continue
        if u.startswith("/state "):
            state_tag = u[len("/state "):].strip()
            print(f"[state tag set: {state_tag}]")
            continue
        if u.startswith("/sys-time "):
            sys_extra = u[len("/sys-time "):].strip()
            print(f"[sys-prompt extra set: {sys_extra}]")
            continue
        if u.startswith("/clear-tags"):
            scene_tag = None
            state_tag = None
            sys_extra = None
            print("[all sys-prompt extras cleared]")
            continue
        if u.startswith("/clear"):
            history = []
            print("[conversation history cleared]")
            continue
        if u.startswith("/show-system"):
            print("--- current system prompt ---")
            print(current_system())
            print("--- end ---")
            continue

        history.append(("user", u))
        print("dave> ", end="", flush=True)
        reply = respond_history(model, tokenizer, current_system(), history)
        history.append(("assistant", reply))
        print()


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--adapter", required=True)
    p.add_argument("--base", default="unsloth/Qwen3.5-4B")
    p.add_argument("--mode", default="all",
                   choices=["all", "baseline", "sys-time", "stage-time"])
    p.add_argument("--interactive", action="store_true")
    args = p.parse_args()

    bnb = BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_compute_dtype=torch.bfloat16,
        bnb_4bit_quant_type="nf4",
    )

    print(f"[load] base={args.base}")
    tokenizer = AutoTokenizer.from_pretrained(args.base, trust_remote_code=True)
    base = AutoModelForCausalLM.from_pretrained(
        args.base,
        quantization_config=bnb,
        device_map="auto",
        trust_remote_code=True,
    )
    print(f"[base] class={type(base).__name__}")
    base_param_names = [n for n, _ in base.named_parameters()]
    print(f"[base] sample param paths: {base_param_names[:3]}")

    # Validate adapter path BEFORE handing to PEFT. Without this, PEFT's
    # PeftModel.from_pretrained falls through to HF Hub on a missing
    # adapter_config.json, producing a confusing HFValidationError about
    # repo-id format. resolve_adapter_path also handles the case where
    # the user pointed at a parent dir and the config is in a checkpoint-*
    # subdir.
    adapter_path = resolve_adapter_path(args.adapter)

    maybe_rename_adapter_keys(adapter_path, base_param_names)

    print(f"[load] adapter={adapter_path}")
    model = PeftModel.from_pretrained(base, adapter_path)
    model.eval()

    print("[mode] enable_thinking=False (Dave does not use <think>)\n")

    if args.interactive:
        run_interactive(model, tokenizer)
    elif args.mode == "all":
        run_all_modes(model, tokenizer)
    else:
        run_canned_test(model, tokenizer, args.mode)


if __name__ == "__main__":
    main()
