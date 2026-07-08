import { useEffect, useState } from 'react';
import { useDaveStore } from '../state/store';
import {
  ipc,
  SETTING_KEY_OUTREACH_THRESHOLD,
  type ModelInfo,
  type PersonaInfo,
} from '../lib/tauri';

// Sentinel values used in the persona dropdown for the two non-file options.
// "(default — built-in)" maps to the in-binary SYSTEM_PROMPT constant.
// "(custom — edited below)" appears only when the textarea diverges from
// every known preset.
const PERSONA_DEFAULT_KEY = '__default__';
const PERSONA_CUSTOM_KEY = '__custom__';

// Hidden behind a gear icon in the StatusBar. Testing/admin surface — not
// part of normal use. Adjusts the outreach idle threshold and provides
// DB ops (inject test conversation, clear all data, export DB).

const MIN_MIN = 1;
const MAX_MIN = 15;
const DEFAULT_MIN = 3; // Locked in 2026-05-01 from Bo's tuned setting.

export function SettingsPanel() {
  const open = useDaveStore((s) => s.settingsPanelOpen);
  const close = useDaveStore((s) => s.closeSettingsPanel);
  const reload = useDaveStore((s) => s.reloadAfterDbReset);
  const toggleMemory = useDaveStore((s) => s.toggleMemoryPanel);
  const toggleDrops = useDaveStore((s) => s.toggleDropsPanel);
  const toggleJournal = useDaveStore((s) => s.toggleJournalPanel);

  function openPanel(toggle: () => void) {
    close();
    setTimeout(toggle, 0);
  }

  const [thresholdMin, setThresholdMin] = useState<number>(DEFAULT_MIN);
  const [savingThreshold, setSavingThreshold] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  const [busy, setBusy] = useState<null | 'inject' | 'clear' | 'export'>(null);
  const [lastResult, setLastResult] = useState<string | null>(null);

  // Chat-triage testing override. Empty = normal weighted-sampling. Set to
  // 'delay' / 'refuse' / 'respond' to force that decision on every send.
  // Useful for verifying the deferred-fire path without rolling probabilistic
  // dice in normal triage.
  const [triageForce, setTriageForce] = useState<string>('');
  const [savingTriage, setSavingTriage] = useState(false);

  // Pace factor — global timing multiplier. 0.2 = snappy, 1.0 = baseline,
  // 2.0 = deliberate. Backend reads this from settings on every send and
  // scales every timing variable proportionally (read delay + compose hold
  // + per-char + punctuation pauses). One knob for the whole system.
  // Default locked in 2026-05-01 from Bo's tuned setting. Mirrors
  // chat_pacing.rs PACE_DEFAULT (0.65).
  const PACE_MIN_F = 0.2;
  const PACE_MAX_F = 2.0;
  const PACE_DEFAULT_F = 0.65;
  const [pace, setPace] = useState<number>(PACE_DEFAULT_F);
  const [savingPace, setSavingPace] = useState(false);

  // Model selector + thinking toggle. Hot-swap is real: switch_model
  // kills the running llama-server and respawns. Expect a 30-90s wait
  // for the larger models. Background workers (idle, outreach,
  // consolidation) keep their LlamaClient pointed at 127.0.0.1:8080 —
  // the port doesn't change, so they recover automatically once the
  // new server is /health-ready.
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [activeModel, setActiveModel] = useState<string>('');
  const [pendingModel, setPendingModel] = useState<string>('');
  const [switchingModel, setSwitchingModel] = useState(false);
  // Default OFF to match the backend canon default. A transient
  // getThinkingEnabled() failure must not paint the checkbox ON (thinking-on
  // is exactly the state the reasoning-format fix exists to prevent by accident).
  const [thinking, setThinking] = useState<boolean>(false);
  const [savingThinking, setSavingThinking] = useState(false);

  // Persona swap state.
  //
  // `activePrompt`     = what the running backend cache currently holds
  //                      (last value successfully sent through setSystemPrompt
  //                      or the default at boot).
  // `editingPrompt`    = the textarea's current content. May diverge from
  //                      active until the user clicks apply.
  // `defaultPrompt`    = the in-binary baseline, fetched once for the
  //                      "(default — built-in)" dropdown option.
  // `personas`         = preset descriptors (default sentinel + every
  //                      *.txt file the backend found in C:\DAVE\personas\).
  // `selectedPersona`  = the dropdown's selected key. One of:
  //                      PERSONA_DEFAULT_KEY, PERSONA_CUSTOM_KEY, or a path.
  const [personas, setPersonas] = useState<PersonaInfo[]>([]);
  const [activePrompt, setActivePrompt] = useState<string>('');
  const [editingPrompt, setEditingPrompt] = useState<string>('');
  const [defaultPrompt, setDefaultPrompt] = useState<string>('');
  const [selectedPersona, setSelectedPersona] = useState<string>(PERSONA_DEFAULT_KEY);
  const [savingPersona, setSavingPersona] = useState(false);
  const [resettingPersona, setResettingPersona] = useState(false);

  useEffect(() => {
    if (!open) return;
    setLastResult(null);
    setConfirmClear(false);
    ipc
      .getSetting(SETTING_KEY_OUTREACH_THRESHOLD)
      .then((raw) => {
        if (raw) {
          const secs = parseInt(raw, 10);
          if (!Number.isNaN(secs)) {
            const minutes = Math.max(MIN_MIN, Math.min(MAX_MIN, Math.round(secs / 60)));
            setThresholdMin(minutes);
          }
        }
      })
      .catch(() => {});
    ipc
      .getSetting('chat_triage_force')
      .then((raw) => setTriageForce(raw || ''))
      .catch(() => {});
    ipc
      .getSetting('chat_pacing_pace')
      .then((raw) => {
        if (raw) {
          const v = parseFloat(raw);
          if (!Number.isNaN(v)) {
            setPace(Math.max(PACE_MIN_F, Math.min(PACE_MAX_F, v)));
          }
        }
      })
      .catch(() => {});
    // Models + active path + thinking-enabled.
    // Errors are surfaced to lastResult so we can see if the IPC command
    // is missing (likely an old dev-binary issue) vs. an empty C:\models.
    ipc
      .listModels()
      .then((m) => {
        setModels(m);
        if (m.length === 0) {
          setLastResult('listModels returned 0 entries — check C:\\models exists');
        }
      })
      .catch((e) => {
        console.error('listModels failed:', e);
        setModels([]);
        setLastResult(`listModels error: ${String(e).slice(0, 200)}`);
      });
    ipc
      .getActiveModel()
      .then((p) => setActiveModel(p || ''))
      .catch((e) => {
        console.error('getActiveModel failed:', e);
        setActiveModel('');
      });
    ipc
      .getThinkingEnabled()
      .then(setThinking)
      .catch((e) => {
        console.error('getThinkingEnabled failed:', e);
        setThinking(false);
      });

    // Persona section — pull the four pieces in parallel:
    //   1) preset list (default + files)
    //   2) the in-binary default text (for the "(default)" option preview)
    //   3) the live active prompt (textarea seed)
    // If any fails, surface to lastResult so the cause is visible (most
    // likely a stale binary missing the new commands).
    Promise.all([
      ipc.listPersonas(),
      ipc.getDefaultSystemPrompt(),
      ipc.getSystemPrompt(),
    ])
      .then(([list, def, active]) => {
        setPersonas(list);
        setDefaultPrompt(def);
        setActivePrompt(active);
        setEditingPrompt(active);
        // Pick the dropdown selection that matches the current active text.
        // Match priority: default first, then any preset whose content equals
        // active, else "custom".
        if (active === def) {
          setSelectedPersona(PERSONA_DEFAULT_KEY);
        } else {
          // We have to fetch each file's content to compare; defer that to
          // when the user opens the dropdown. Default to "custom" for now.
          setSelectedPersona(PERSONA_CUSTOM_KEY);
        }
      })
      .catch((e) => {
        console.error('persona load failed:', e);
        setLastResult(`persona load error: ${String(e).slice(0, 200)}`);
      });
  }, [open]);

  async function handleTriageForceChange(v: string) {
    setTriageForce(v);
    setSavingTriage(true);
    try {
      await ipc.setSetting('chat_triage_force', v);
    } catch (e) {
      console.error('save triage force failed:', e);
    } finally {
      setSavingTriage(false);
    }
  }

  async function handlePaceChange(v: number) {
    setPace(v);
    setSavingPace(true);
    try {
      await ipc.setSetting('chat_pacing_pace', v.toFixed(2));
    } catch (e) {
      console.error('save pace failed:', e);
    } finally {
      setSavingPace(false);
    }
  }

  function fmtSize(bytes: number): string {
    if (!bytes) return '';
    const gb = bytes / (1024 * 1024 * 1024);
    if (gb >= 1) return `${gb.toFixed(1)} GB`;
    const mb = bytes / (1024 * 1024);
    return `${mb.toFixed(0)} MB`;
  }

  async function handleApplyModel() {
    const target = pendingModel || activeModel;
    if (!target || target === activeModel) return;
    setSwitchingModel(true);
    setLastResult(null);
    try {
      const loaded = await ipc.switchModel(target);
      setActiveModel(loaded);
      setPendingModel('');
      // refresh list to update `active` flags
      const fresh = await ipc.listModels();
      setModels(fresh);
      const name = loaded.split(/[\\/]/).pop() || loaded;
      setLastResult(`Loaded: ${name}`);
    } catch (e) {
      setLastResult(`Switch failed: ${e}`);
    } finally {
      setSwitchingModel(false);
    }
  }

  async function handleThinkingToggle(next: boolean) {
    setThinking(next);
    setSavingThinking(true);
    try {
      await ipc.setThinkingEnabled(next);
      // The flag is read at the next spawn — to apply now, we must
      // re-spawn with the same model. switch_model with the active
      // path does that.
      if (activeModel) {
        setSwitchingModel(true);
        const loaded = await ipc.switchModel(activeModel);
        setActiveModel(loaded);
        setLastResult(
          `Thinking ${next ? 'enabled' : 'disabled'} (server restarted).`
        );
      } else {
        setLastResult(
          `Thinking ${next ? 'enabled' : 'disabled'} (will apply on next spawn).`
        );
      }
    } catch (e) {
      // Roll the optimistic checkbox back — the server did not actually change.
      setThinking(!next);
      setLastResult(`Thinking toggle failed: ${e}`);
    } finally {
      // Reset BOTH flags here. Previously setSwitchingModel(false) lived only
      // on the success path, so a failed switchModel wedged the entire
      // model/persona section (every control is disabled while switching).
      setSavingThinking(false);
      setSwitchingModel(false);
    }
  }

  // Persona dropdown change — load the selected preset's text into the
  // textarea (preview only, not saved). Picking "(custom)" leaves the
  // textarea alone (the user is mid-edit).
  async function handlePersonaSelect(key: string) {
    setSelectedPersona(key);
    setLastResult(null);
    if (key === PERSONA_CUSTOM_KEY) {
      // No-op — keep current textarea content.
      return;
    }
    if (key === PERSONA_DEFAULT_KEY) {
      setEditingPrompt(defaultPrompt);
      return;
    }
    // It's a file path — fetch content.
    try {
      const text = await ipc.loadPersonaText(key);
      setEditingPrompt(text);
    } catch (e) {
      setLastResult(`Load persona failed: ${e}`);
    }
  }

  // Apply the textarea content as the new live prompt. Persists to DB and
  // updates the live cache; the next inference (chat / idle / outreach /
  // consolidation / departure / startup) will use it.
  async function handlePersonaApply() {
    if (editingPrompt.trim() === '') {
      setLastResult('Persona text cannot be empty (use "reset to default").');
      return;
    }
    if (editingPrompt === activePrompt) {
      setLastResult('No change — that prompt is already active.');
      return;
    }
    setSavingPersona(true);
    setLastResult(null);
    try {
      await ipc.setSystemPrompt(editingPrompt);
      setActivePrompt(editingPrompt);
      setLastResult(
        `Persona applied (${editingPrompt.length} chars). Active on next message.`
      );
      // Update dropdown label: if textarea matches default, snap selector to default.
      if (editingPrompt === defaultPrompt) {
        setSelectedPersona(PERSONA_DEFAULT_KEY);
      } else {
        setSelectedPersona(PERSONA_CUSTOM_KEY);
      }
    } catch (e) {
      setLastResult(`Persona apply failed: ${e}`);
    } finally {
      setSavingPersona(false);
    }
  }

  // Revert to the in-binary default. Clears the DB override and reseats
  // the cache. Useful when a custom prompt is misbehaving.
  async function handlePersonaReset() {
    setResettingPersona(true);
    setLastResult(null);
    try {
      const restored = await ipc.resetSystemPrompt();
      setActivePrompt(restored);
      setEditingPrompt(restored);
      setSelectedPersona(PERSONA_DEFAULT_KEY);
      setLastResult('Reverted to built-in default.');
    } catch (e) {
      setLastResult(`Reset failed: ${e}`);
    } finally {
      setResettingPersona(false);
    }
  }

  if (!open) return null;

  async function handleSliderChange(v: number) {
    setThresholdMin(v);
    setSavingThreshold(true);
    try {
      await ipc.setSetting(SETTING_KEY_OUTREACH_THRESHOLD, String(v * 60));
    } catch (e) {
      console.error('save threshold failed:', e);
    } finally {
      setSavingThreshold(false);
    }
  }

  async function handleInject() {
    setBusy('inject');
    setLastResult(null);
    try {
      await ipc.injectTestConversation();
      await reload();
      setLastResult('Test conversation injected.');
    } catch (e) {
      setLastResult(`Inject failed: ${e}`);
    } finally {
      setBusy(null);
    }
  }

  async function handleClear() {
    if (!confirmClear) {
      setConfirmClear(true);
      return;
    }
    setBusy('clear');
    setLastResult(null);
    try {
      await ipc.clearAllData();
      await reload();
      setLastResult('All data cleared.');
      setConfirmClear(false);
    } catch (e) {
      setLastResult(`Clear failed: ${e}`);
    } finally {
      setBusy(null);
    }
  }

  async function handleExport() {
    setBusy('export');
    setLastResult(null);
    try {
      const path = await ipc.exportDatabase();
      setLastResult(`Exported to: ${path}`);
    } catch (e) {
      setLastResult(`Export failed: ${e}`);
    } finally {
      setBusy(null);
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex"
      style={{ backgroundColor: 'rgba(0, 0, 0, 0.4)' }}
      onClick={close}
    >
      <aside
        className="h-full overflow-y-auto p-10"
        style={{
          backgroundColor: 'var(--bg-elevated)',
          width: '420px',
          borderRight: '1px solid var(--border-medium)',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="status-bar mb-8" style={{ opacity: 0.6 }}>
          settings
        </div>

        {/* Model — hot-swap the loaded GGUF + thinking-mode toggle */}
        <div className="mb-10">
          <div
            className="status-bar mb-2"
            style={{ color: 'var(--text-secondary)', textTransform: 'none' }}
          >
            model
          </div>
          <div
            className="status-bar mb-3"
            style={{ opacity: 0.5, fontSize: 11, textTransform: 'none' }}
          >
            switch the loaded GGUF. apply restarts llama-server (~30-90s).
            background workers (idle, outreach, consolidation) reconnect
            automatically.
          </div>
          <select
            value={pendingModel || activeModel}
            onChange={(e) => setPendingModel(e.target.value)}
            disabled={switchingModel || savingThinking}
            className="settings-button"
            style={{ width: '100%', marginBottom: 8 }}
          >
            {models.length === 0 && (
              <option value="">(no .gguf files found in C:\models)</option>
            )}
            {models.map((m) => {
              const sz = fmtSize(m.size_bytes);
              const label = sz ? `${m.name} (${sz})` : m.name;
              return (
                <option key={m.path} value={m.path}>
                  {m.active ? '● ' : '   '}
                  {label}
                </option>
              );
            })}
          </select>
          <button
            className="settings-button"
            onClick={handleApplyModel}
            disabled={
              switchingModel ||
              savingThinking ||
              !pendingModel ||
              pendingModel === activeModel
            }
            style={{ width: '100%' }}
          >
            {switchingModel
              ? 'restarting llama-server…'
              : pendingModel && pendingModel !== activeModel
              ? 'apply'
              : 'apply (no change)'}
          </button>
          <label
            className="status-bar"
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 8,
              marginTop: 12,
              textTransform: 'none',
              cursor: savingThinking || switchingModel ? 'wait' : 'pointer',
            }}
          >
            <input
              type="checkbox"
              checked={thinking}
              disabled={savingThinking || switchingModel}
              onChange={(e) => handleThinkingToggle(e.target.checked)}
            />
            <span style={{ color: 'var(--text-primary)' }}>
              thinking enabled
            </span>
            <span style={{ opacity: 0.5, fontSize: 11 }}>
              (Qwen3.5 small variants ship with thinking off — toggle
              applies on restart)
            </span>
          </label>
          {activeModel && (
            <div
              className="status-bar"
              style={{
                opacity: 0.4,
                fontSize: 10,
                textTransform: 'none',
                marginTop: 8,
                wordBreak: 'break-all',
              }}
            >
              active: {activeModel.split(/[\\/]/).pop()}
            </div>
          )}
        </div>

        {/* Persona — hot-swap the system prompt that defines Dave's voice.
            The dropdown previews preset .txt files from C:\DAVE\personas\;
            the textarea is editable and is what actually gets applied. No
            restart needed — the change takes effect on the next message. */}
        <div className="mb-10">
          <div
            className="status-bar mb-2"
            style={{ color: 'var(--text-secondary)', textTransform: 'none' }}
          >
            persona
          </div>
          <div
            className="status-bar mb-3"
            style={{ opacity: 0.5, fontSize: 11, textTransform: 'none' }}
          >
            swap the system prompt. selecting a preset previews it below;
            apply makes it live. takes effect on the next message — chat,
            idle worker, outreach, consolidation, departure, startup all
            read from the same cache. drop new presets as *.txt in the
            app's personas folder and reopen this panel to pick them up.
          </div>
          <select
            value={selectedPersona}
            onChange={(e) => handlePersonaSelect(e.target.value)}
            disabled={savingPersona || resettingPersona}
            className="settings-button"
            style={{ width: '100%', marginBottom: 8 }}
          >
            <option value={PERSONA_DEFAULT_KEY}>
              {selectedPersona === PERSONA_DEFAULT_KEY &&
              activePrompt === defaultPrompt
                ? '● '
                : '   '}
              (default — built-in) ({defaultPrompt.length} chars)
            </option>
            <option value={PERSONA_CUSTOM_KEY}>
              {selectedPersona === PERSONA_CUSTOM_KEY ? '● ' : '   '}
              (custom — edited below)
            </option>
            {personas
              .filter((p) => !p.is_default && p.path)
              .map((p) => (
                <option key={p.path!} value={p.path!}>
                  {selectedPersona === p.path ? '● ' : '   '}
                  {p.name} ({p.char_count} chars)
                </option>
              ))}
          </select>
          <textarea
            value={editingPrompt}
            onChange={(e) => {
              setEditingPrompt(e.target.value);
              // Any edit means "custom" unless it happens to match a known
              // option exactly. Keep dropdown honest.
              if (e.target.value === defaultPrompt) {
                setSelectedPersona(PERSONA_DEFAULT_KEY);
              } else {
                setSelectedPersona(PERSONA_CUSTOM_KEY);
              }
            }}
            disabled={savingPersona || resettingPersona}
            spellCheck={false}
            style={{
              width: '100%',
              minHeight: 220,
              maxHeight: 420,
              resize: 'vertical',
              fontFamily:
                'ui-monospace, "Cascadia Mono", "Consolas", monospace',
              fontSize: 11,
              lineHeight: 1.5,
              padding: 8,
              backgroundColor: 'var(--bg-base)',
              color: 'var(--text-primary)',
              border: '1px solid var(--border-medium)',
              borderRadius: 2,
              marginBottom: 8,
            }}
          />
          <div style={{ display: 'flex', gap: 8 }}>
            <button
              className="settings-button"
              onClick={handlePersonaApply}
              disabled={
                savingPersona ||
                resettingPersona ||
                editingPrompt.trim() === '' ||
                editingPrompt === activePrompt
              }
              style={{ flex: 1 }}
            >
              {savingPersona
                ? 'applying…'
                : editingPrompt === activePrompt
                ? 'apply (no change)'
                : `apply (${editingPrompt.length} chars)`}
            </button>
            <button
              className="settings-button"
              onClick={handlePersonaReset}
              disabled={savingPersona || resettingPersona}
              style={{ flex: 1 }}
              title="Clear the override. Reverts to the SYSTEM_PROMPT constant baked into the binary."
            >
              {resettingPersona ? 'resetting…' : 'reset to default'}
            </button>
          </div>
          <div
            className="status-bar"
            style={{
              opacity: 0.4,
              fontSize: 10,
              textTransform: 'none',
              marginTop: 8,
            }}
          >
            active: {activePrompt.length} chars
            {activePrompt !== editingPrompt && (
              <span style={{ color: '#d7826b', marginLeft: 8 }}>
                (textarea has unsaved edits)
              </span>
            )}
          </div>
        </div>

        {/* Outreach threshold */}
        <div className="mb-10">
          <div
            className="status-bar mb-2"
            style={{ color: 'var(--text-secondary)', textTransform: 'none' }}
          >
            outreach idle threshold
          </div>
          <div
            className="status-bar mb-3"
            style={{ opacity: 0.5, fontSize: 11, textTransform: 'none' }}
          >
            how long the user must be quiet before Dave is given the floor.
          </div>
          <div className="flex items-center gap-4">
            <input
              type="range"
              min={MIN_MIN}
              max={MAX_MIN}
              step={1}
              value={thresholdMin}
              onChange={(e) => handleSliderChange(parseInt(e.target.value, 10))}
              style={{ flex: 1 }}
              disabled={savingThreshold}
            />
            <span
              className="status-bar"
              style={{ color: 'var(--text-primary)', minWidth: 70, textAlign: 'right' }}
            >
              {thresholdMin} min
            </span>
          </div>
        </div>

        {/* Pace — global timing multiplier */}
        <div className="mb-10">
          <div
            className="status-bar mb-2"
            style={{ color: 'var(--text-secondary)', textTransform: 'none' }}
          >
            pace
          </div>
          <div
            className="status-bar mb-3"
            style={{ opacity: 0.5, fontSize: 11, textTransform: 'none' }}
          >
            global timing multiplier. scales every delay proportionally:
            read time, compose hold, per-char streaming, punctuation pauses.
            0.2x = snappy. 1.0x = baseline. 2.0x = deliberate. one knob for
            the whole feel. takes effect on next send (no restart needed).
          </div>
          <div className="flex items-center gap-4">
            <input
              type="range"
              min={PACE_MIN_F}
              max={PACE_MAX_F}
              step={0.05}
              value={pace}
              onChange={(e) => handlePaceChange(parseFloat(e.target.value))}
              style={{ flex: 1 }}
              disabled={savingPace}
            />
            <span
              className="status-bar"
              style={{ color: 'var(--text-primary)', minWidth: 70, textAlign: 'right' }}
            >
              {pace.toFixed(2)}x
            </span>
          </div>
        </div>

        {/* Chat triage force-override (testing) */}
        <div className="mb-10">
          <div
            className="status-bar mb-2"
            style={{ color: 'var(--text-secondary)', textTransform: 'none' }}
          >
            chat triage override (testing)
          </div>
          <div
            className="status-bar mb-3"
            style={{ opacity: 0.5, fontSize: 11, textTransform: 'none' }}
          >
            force every send_to_dave to take this branch. blank = normal
            weighted-sampling triage. delay = 5s deferred fire (chat pacing
            applies on top). refuse = no response. respond = normal
            immediate reply.
          </div>
          <select
            value={triageForce}
            onChange={(e) => handleTriageForceChange(e.target.value)}
            disabled={savingTriage}
            className="settings-button"
            style={{ width: '100%' }}
          >
            <option value="">normal (weighted sampling)</option>
            <option value="respond">force respond</option>
            <option value="delay">force delay (5s)</option>
            <option value="refuse">force refuse</option>
          </select>
        </div>

        {/* Panels — open without remembering keyboard shortcuts */}
        <div className="mb-10">
          <div
            className="status-bar mb-3"
            style={{ color: 'var(--text-secondary)', textTransform: 'none' }}
          >
            panels
          </div>
          <div className="flex flex-col gap-2">
            <button
              className="settings-button"
              onClick={() => openPanel(toggleMemory)}
            >
              memory inspector <span style={{ opacity: 0.4, marginLeft: 6 }}>(Ctrl+Shift+M)</span>
            </button>
            <button
              className="settings-button"
              onClick={() => openPanel(toggleDrops)}
            >
              outreach drops <span style={{ opacity: 0.4, marginLeft: 6 }}>(Ctrl+Shift+J)</span>
            </button>
            <button
              className="settings-button"
              onClick={() => openPanel(toggleJournal)}
            >
              journal <span style={{ opacity: 0.4, marginLeft: 6 }}>(Ctrl+J)</span>
            </button>
          </div>
        </div>

        {/* Database ops */}
        <div className="mb-10">
          <div
            className="status-bar mb-3"
            style={{ color: 'var(--text-secondary)', textTransform: 'none' }}
          >
            database
          </div>
          <div className="flex flex-col gap-2">
            <button
              className="settings-button"
              onClick={handleInject}
              disabled={busy !== null}
            >
              {busy === 'inject' ? 'injecting…' : 'inject test conversation'}
            </button>
            <button
              className="settings-button"
              onClick={handleExport}
              disabled={busy !== null}
            >
              {busy === 'export' ? 'exporting…' : 'export database'}
            </button>
            <button
              className="settings-button"
              onClick={handleClear}
              disabled={busy !== null}
              style={{
                color: confirmClear ? '#d7826b' : undefined,
                borderColor: confirmClear ? '#d7826b' : undefined,
              }}
            >
              {busy === 'clear'
                ? 'clearing…'
                : confirmClear
                ? 'confirm: wipe everything'
                : 'clear all data'}
            </button>
          </div>
        </div>

        {lastResult && (
          <div
            className="status-bar"
            style={{
              color: 'var(--text-secondary)',
              textTransform: 'none',
              fontSize: 12,
              opacity: 0.85,
              wordBreak: 'break-all',
            }}
          >
            {lastResult}
          </div>
        )}

        <div
          className="status-bar mt-12"
          style={{ opacity: 0.35, textTransform: 'none', fontSize: 11 }}
        >
          esc or click outside to close
        </div>
      </aside>
      <div className="flex-1" />
    </div>
  );
}
