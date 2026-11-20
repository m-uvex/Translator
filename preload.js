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
  saveFile: (name, data, kind) => ipcRenderer.invoke('file:save', name, data, kind),
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
  nllbUrl: u => ipcRenderer.send('nllb:url', u),
  // Dieselbe Meldung fuer Ollama: der Hauptprozess prueft beim Hochfahren die
  // eingestellte Adresse und startet nur einen lokalen Server.
  ollamaUrl: u => ipcRenderer.send('ollama:url', u),
  onRequestClose: cb => ipcRenderer.on('app:request-close', () => cb()),
  // Ob der systemweite Kurzbefehl wirklich registriert wurde (ein anderes
  // Programm kann ihn belegt haben); die Einstellungen richten sich danach.
  hotkeyOk: () => ipcRenderer.invoke('app:hotkey'),
  forceQuit: () => ipcRenderer.invoke('app:forceQuit'),
  winMinimize: () => ipcRenderer.invoke('win:minimize'),
  winToggleMax: () => ipcRenderer.invoke('win:toggleMax'),
  winClose: () => ipcRenderer.invoke('win:close'),
  onWinState: cb => ipcRenderer.on('win:state', (_e, max) => cb(max))
});