import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("simBridge", {
  getInitialState: () => ipcRenderer.invoke("sim:get-initial-state"),
  subscribeSnapshots: (listener: (snapshot: unknown) => void) => {
    const wrapped = (_event: Electron.IpcRendererEvent, snapshot: unknown) => {
      listener(snapshot);
    };
    ipcRenderer.on("sim:snapshot", wrapped);
    return () => {
      ipcRenderer.removeListener("sim:snapshot", wrapped);
    };
  },
  sendPlayerCommand: (command: unknown) => ipcRenderer.invoke("sim:send-command", command),
  reloadMods: (recoverToDefault: boolean) => ipcRenderer.invoke("sim:reload-mods", Boolean(recoverToDefault)),
  listAvailableSkins: () => ipcRenderer.invoke("sim:list-skins"),
  readSkinAssetManifest: (skinId: string) => ipcRenderer.invoke("sim:read-skin-manifest", String(skinId)),
  loadMockFixture: (fixtureJson: string) => ipcRenderer.invoke("sim:load-mock-fixture", String(fixtureJson)),
  subscribeSkinHotReload: (listener: (skinId: string) => void) => {
    const wrapped = (_event: Electron.IpcRendererEvent, skinId: unknown) => {
      listener(String(skinId));
    };
    ipcRenderer.on("sim:skin-hot-reload", wrapped);
    return () => {
      ipcRenderer.removeListener("sim:skin-hot-reload", wrapped);
    };
  },
  readUserPreferences: () => ipcRenderer.invoke("sim:read-user-prefs"),
  writeUserPreferences: (prefs: unknown) => ipcRenderer.invoke("sim:write-user-prefs", prefs),
});
