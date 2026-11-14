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
});
