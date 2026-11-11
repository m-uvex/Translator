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
  fontsList: () => ipcRenderer.invoke('fonts:list'),
  onRequestClose: cb => ipcRenderer.on('app:request-close', () => cb()),
  forceQuit: () => ipcRenderer.invoke('app:forceQuit')
});