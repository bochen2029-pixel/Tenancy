import { useEffect, useState } from 'react';
import { useDaveStore } from '../state/store';
import { ipc, SETTING_KEY_OUTREACH_THRESHOLD } from '../lib/tauri';

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
