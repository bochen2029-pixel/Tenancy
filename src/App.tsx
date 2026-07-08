import { useEffect } from 'react';
import { useDaveStore } from './state/store';
import { StatusBar } from './components/StatusBar';
import { Conversation } from './components/Conversation';
import { Composer } from './components/Composer';
import { JournalPanel } from './components/JournalPanel';
import { DropsPanel } from './components/DropsPanel';
import { SettingsPanel } from './components/SettingsPanel';
import { MemoryInspector } from './components/MemoryInspector';

export default function App() {
  const ready = useDaveStore((s) => s.ready);
  const initError = useDaveStore((s) => s.initError);
  const togglePanel = useDaveStore((s) => s.toggleJournalPanel);
  const journalOpen = useDaveStore((s) => s.journalPanelOpen);
  const closeJournal = useDaveStore((s) => s.closeJournalPanel);
  const toggleDrops = useDaveStore((s) => s.toggleDropsPanel);
  const dropsOpen = useDaveStore((s) => s.dropsPanelOpen);
  const closeDrops = useDaveStore((s) => s.closeDropsPanel);
  const toggleSettings = useDaveStore((s) => s.toggleSettingsPanel);
  const settingsOpen = useDaveStore((s) => s.settingsPanelOpen);
  const closeSettings = useDaveStore((s) => s.closeSettingsPanel);
  const toggleMemory = useDaveStore((s) => s.toggleMemoryPanel);
  const memoryOpen = useDaveStore((s) => s.memoryPanelOpen);
  const closeMemory = useDaveStore((s) => s.closeMemoryPanel);
  const flash = useDaveStore((s) => s.flash);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      // Ctrl+,        → settings panel (gear icon also opens it)
      // Ctrl+Shift+M  → memory inspector (read/edit/history)
      // Ctrl+Shift+J  → drops panel (forensic, hidden surface)
      // Ctrl+J        → journal panel (Dave's writing)
      if (e.ctrlKey && e.key === ',') {
        e.preventDefault();
        toggleSettings();
      } else if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 'm') {
        e.preventDefault();
        toggleMemory();
      } else if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 'j') {
        e.preventDefault();
        toggleDrops();
      } else if (e.ctrlKey && e.key.toLowerCase() === 'j') {
        e.preventDefault();
        togglePanel();
      } else if (e.ctrlKey && e.altKey && e.code === 'ArrowUp') {
        // Curation gesture (invisible): his last reach felt right.
        e.preventDefault();
        useDaveStore.getState().rateLastReach(1);
      } else if (e.ctrlKey && e.altKey && e.code === 'ArrowDown') {
        // His last reach felt wrong.
        e.preventDefault();
        useDaveStore.getState().rateLastReach(-1);
      } else if (e.ctrlKey && e.altKey && e.code === 'KeyM') {
        // He should have reached here and didn't.
        e.preventDefault();
        useDaveStore.getState().markMissedReach();
      } else if (e.key === 'Escape') {
        if (memoryOpen) closeMemory();
        else if (settingsOpen) closeSettings();
        else if (dropsOpen) closeDrops();
        else if (journalOpen) closeJournal();
      }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [
    togglePanel, journalOpen, closeJournal,
    toggleDrops, dropsOpen, closeDrops,
    toggleSettings, settingsOpen, closeSettings,
    toggleMemory, memoryOpen, closeMemory,
  ]);

  if (initError && ready) {
    return (
      <div className="h-screen flex flex-col">
        <StatusBar />
        <div className="flex-1 flex items-center justify-center px-12">
          <p
            className="dave-body text-center"
            style={{
              color: 'var(--text-secondary)',
              fontStyle: 'italic',
              maxWidth: '36ch',
            }}
          >
            {initError}
          </p>
        </div>
        <JournalPanel />
        <DropsPanel />
        <SettingsPanel />
        <MemoryInspector />
      </div>
    );
  }

  return (
    <div className="h-screen flex flex-col">
      <StatusBar />
      {ready ? (
        <>
          <Conversation />
          <Composer />
        </>
      ) : (
        <div className="flex-1" />
      )}
      {flash && (
        <div
          aria-hidden
          style={{
            position: 'fixed',
            bottom: 74,
            left: 0,
            right: 0,
            textAlign: 'center',
            pointerEvents: 'none',
            fontStyle: 'italic',
            fontSize: 12,
            letterSpacing: '0.06em',
            color: 'var(--text-fade)',
          }}
        >
          {flash}
        </div>
      )}
      <JournalPanel />
      <DropsPanel />
      <SettingsPanel />
      <MemoryInspector />
    </div>
  );
}
