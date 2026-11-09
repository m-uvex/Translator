const { contextBridge, ipcRenderer } = require('electron');
contextBridge.exposeInMainWorld('api', {
  extract: (name, data) => ipcRenderer.invoke('extract', name, data),
  spellCheck: (lang, words) => ipcRenderer.invoke('spell:check', lang, words),
  spellSuggest: (lang, word) => ipcRenderer.invoke('spell:suggest', lang, word),
  spellAdd: word => ipcRenderer.invoke('spell:add', word),
  saveFile: (name, data, kind) => ipcRenderer.invoke('file:save', name, data, kind),
  fontsList: () => ipcRenderer.invoke('fonts:list')
});