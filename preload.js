const { contextBridge, ipcRenderer } = require('electron');
contextBridge.exposeInMainWorld('api', {
  extract: (name, data) => ipcRenderer.invoke('extract', name, data),
  spellCheck: (lang, words) => ipcRenderer.invoke('spell:check', lang, words),
  spellSuggest: (lang, word) => ipcRenderer.invoke('spell:suggest', lang, word),
  spellAdd: word => ipcRenderer.invoke('spell:add', word),
  spellGetPersonal: () => ipcRenderer.invoke('spell:getPersonal'),
  spellRemove: word => ipcRenderer.invoke('spell:remove', word),
  spellEdit: (oldWord, newWord) => ipcRenderer.invoke('spell:edit', oldWord, newWord),
  dictExport: () => ipcRenderer.invoke('dict:export'),
  dictImport: () => ipcRenderer.invoke('dict:import'),
  saveFile: (name, data, kind, opts) => ipcRenderer.invoke('file:save', name, data, kind, opts),
  libList: () => ipcRenderer.invoke('lib:list'),
  libRead: id => ipcRenderer.invoke('lib:read', id),
  libSave: doc => ipcRenderer.invoke('lib:save', doc),
  libDelete: id => ipcRenderer.invoke('lib:delete', id),
  studyLoad: name => ipcRenderer.invoke('study:load', name),
  studySave: (name, data) => ipcRenderer.invoke('study:save', name, data),
  studyImportText: () => ipcRenderer.invoke('study:importText'),
  clipText: () => ipcRenderer.invoke('clip:readText'),
  clipWrite: text => ipcRenderer.invoke('clip:writeText', text),
  studyImportFile: ext => ipcRenderer.invoke('study:importFile', ext),
  studyExportFile: (name, data, ext) => ipcRenderer.invoke('study:exportFile', name, data, ext),
  fontsList: () => ipcRenderer.invoke('fonts:list'),
  // Protokoll und Datenpruefung: Zeilen aus der Oberflaeche ablegen, die letzten
  // Zeilen holen bzw. in die Zwischenablage legen, und beim Start nachsehen, ob
  // eine Datei unfertig oder nicht lesbar ist.
  logWrite: msg => ipcRenderer.invoke('log:write', msg),
  logTail: n => ipcRenderer.invoke('log:tail', n),
  logCopy: () => ipcRenderer.invoke('log:copy'),
  logPath: () => ipcRenderer.invoke('log:path'),
  dataCheck: () => ipcRenderer.invoke('data:check'),
  nllbUrl: u => ipcRenderer.send('nllb:url', u),
  // Python-Umgebung und Pakete aus der App heraus einrichten (Einstellungen,
  // zusammen mit einem Modell-Download). Liefert {ok, error?}.
  nllbSetup: () => ipcRenderer.invoke('nllb:setup'),
  onNllbSetup: cb => { const f = (_e, d) => cb(d); ipcRenderer.on('nllb:setup', f); return () => ipcRenderer.removeListener('nllb:setup', f); },
  // Einrichtung beim ersten Start: install.ps1 steckt in der Setup-EXE und
  // laeuft hier ohne eigenes Fenster; jede Zeile kommt ueber onSetupLine an.
  setupNeeded: () => ipcRenderer.invoke('setup:needed'),
  setupRun: () => ipcRenderer.invoke('setup:run'),
  setupSkip: () => ipcRenderer.invoke('setup:skip'),
  onSetupLine: cb => { const f = (_e, d) => cb(d); ipcRenderer.on('setup:line', f); return () => ipcRenderer.removeListener('setup:line', f); },
  // Dieselbe Meldung fuer Ollama: der Hauptprozess prueft beim Hochfahren die
  // eingestellte Adresse und startet nur einen lokalen Server.
  ollamaUrl: u => ipcRenderer.send('ollama:url', u),
  onRequestClose: cb => ipcRenderer.on('app:request-close', () => cb()),
  // Ob der systemweite Kurzbefehl wirklich registriert wurde (ein anderes
  // Programm kann ihn belegt haben); die Einstellungen richten sich danach.
  hotkeyOk: () => ipcRenderer.invoke('app:hotkey'),
  // Vorlesen: Chromium hat in Electron keine Stimmen, deshalb spricht der
  // Hauptprozess ueber die Ausgabe des Systems (SAPI bzw. spd-say/espeak-ng).
  ttsSpeak: (text, lang, voice) => ipcRenderer.invoke('tts:speak', text, lang, voice),
  ttsStop: () => ipcRenderer.invoke('tts:stop'),
  // Herunterladbare Stimmen (Piper): Liste, Einrichten mit Fortschritt, Loeschen.
  ttsList: () => ipcRenderer.invoke('tts:list'),
  ttsInstall: id => ipcRenderer.invoke('tts:install', id),
  ttsDelete: id => ipcRenderer.invoke('tts:delete', id),
  onTtsProgress: cb => { const f = (_e, d) => cb(d); ipcRenderer.on('tts:progress', f); return () => ipcRenderer.removeListener('tts:progress', f); },
  forceQuit: () => ipcRenderer.invoke('app:forceQuit'),
  winMinimize: () => ipcRenderer.invoke('win:minimize'),
  winToggleMax: () => ipcRenderer.invoke('win:toggleMax'),
  winClose: () => ipcRenderer.invoke('win:close'),
  onWinState: cb => ipcRenderer.on('win:state', (_e, max) => cb(max))
});