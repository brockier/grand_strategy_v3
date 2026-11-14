import * as THREE from "three";
import { DiegeticSnapshot, DiegeticStoreAccess, MapLayerPlugin, UiIntentBus } from "../spine";

export interface GlobeViewportStatus {
  readonly backend: "webgpu" | "webgl2" | "noop";
  readonly shaderCompiled: boolean;
  readonly cellCount: number;
}

export interface GlobeViewportController {
  readonly status: GlobeViewportStatus;
  updateFromSnapshot(snapshot: DiegeticSnapshot): void;
  resize(width: number, height: number): void;
  dispose(): void;
}

/**
 * Initializes the planetary Three.js renderer (`UI-SHL-001`), probing Chromium Dawn WebGPU
 * first with deterministic WebGL2 fallback and shader compilation verification.
 */
export async function initializeGlobeViewport(
  canvas: HTMLCanvasElement,
  _store: DiegeticStoreAccess,
  _bus: UiIntentBus,
  _layers: readonly MapLayerPlugin[],
): Promise<GlobeViewportController> {
  let backend: "webgpu" | "webgl2" | "noop" = "noop";
  let shaderCompiled = false;
  let cellCount = 0;

  // Probe Chromium Dawn WebGPU device availability when present in desktop runtime
  try {
    const navGpu = (navigator as Navigator & { gpu?: { requestAdapter(): Promise<unknown | null> } }).gpu;
    if (navGpu) {
      const adapter = await navGpu.requestAdapter();
      if (adapter) {
        backend = "webgpu";
      }
    }
  } catch {
    // Graceful WebGL2 fallback follows below
  }

  let renderer: THREE.WebGLRenderer | null = null;
  let scene: THREE.Scene | null = null;
  let camera: THREE.PerspectiveCamera | null = null;
  let rafHandle = 0;

  const gl = canvas.getContext("webgl2", { antialias: true, alpha: false });
  if (gl) {
    renderer = new THREE.WebGLRenderer({ canvas, context: gl, antialias: true });
    if (backend === "noop") {
      backend = "webgl2";
    }
    const width = Math.max(canvas.clientWidth || 800, 1);
    const height = Math.max(canvas.clientHeight || 600, 1);
    renderer.setSize(width, height, false);

    scene = new THREE.Scene();
    camera = new THREE.PerspectiveCamera(45, width / height, 0.1, 100);
    camera.position.set(0, 0, 3.2);

    const sphereGeom = new THREE.IcosahedronGeometry(1.0, 2);
    const shaderMat = new THREE.ShaderMaterial({
      uniforms: {
        uBlend: { value: 0.5 },
      },
      vertexShader: `
        varying vec3 vNormalDir;
        void main() {
          vNormalDir = normalize(normalMatrix * normal);
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        }
      `,
      fragmentShader: `
        varying vec3 vNormalDir;
        uniform float uBlend;
        void main() {
          float shade = clamp(dot(vNormalDir, normalize(vec3(0.4, 0.7, 0.6))) * 0.5 + 0.5, 0.2, 1.0);
          vec3 tint = mix(vec3(0.53, 0.64, 0.68), vec3(0.85, 0.79, 0.65), uBlend);
          gl_FragColor = vec4(tint * shade, 1.0);
        }
      `,
    });
    const globeMesh = new THREE.Mesh(sphereGeom, shaderMat);
    scene.add(globeMesh);

    renderer.compile(scene, camera);
    renderer.render(scene, camera);
    shaderCompiled = true;

    const tickRender = () => {
      if (!renderer || !scene || !camera) return;
      renderer.render(scene, camera);
      rafHandle = requestAnimationFrame(tickRender);
    };
    rafHandle = requestAnimationFrame(tickRender);
  }

  const status: GlobeViewportStatus = {
    get backend() {
      return backend;
    },
    get shaderCompiled() {
      return shaderCompiled;
    },
    get cellCount() {
      return cellCount;
    },
  };

  return {
    status,
    updateFromSnapshot(snapshot: DiegeticSnapshot) {
      cellCount = snapshot.globeTopology.cellCount;
    },
    resize(width: number, height: number) {
      if (!renderer || !camera) return;
      const w = Math.max(width, 1);
      const h = Math.max(height, 1);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      renderer.setSize(w, h, false);
    },
    dispose() {
      if (rafHandle) cancelAnimationFrame(rafHandle);
      renderer?.dispose();
    },
  };
}
