import * as THREE from "three";
import {
  DiegeticSnapshot,
  GlobePalette,
  KnownCellDto,
  ObservedWeatherDto,
  RgbTuple,
} from "../spine";

export const CELL_DATA_TEX_SIZE = 128;

export interface GlobeShaderController {
  readonly material: THREE.ShaderMaterial;
  readonly activeTextureIndex: () => number;
  uploadSnapshot(
    snapshot: DiegeticSnapshot,
    selectedCellId: number | null,
    hoveredCellId: number | null,
  ): void;
  setPalette(palette: GlobePalette): void;
  setCartographicState(
    blend01: number,
    sunDir: THREE.Vector3,
    hoveredCellId: number | null,
    selectedCellId: number | null,
    activeWeatherOverlay: boolean,
  ): void;
  dispose(): void;
}

function rgbTupleToVec3(rgb: RgbTuple): THREE.Vector3 {
  return new THREE.Vector3(rgb[0] / 255, rgb[1] / 255, rgb[2] / 255);
}

function createNeutralPatternTexture(r: number, g: number, b: number): THREE.DataTexture {
  const data = new Uint8Array([
    r, g, b, 255,
    Math.max(0, r - 12), Math.max(0, g - 12), Math.max(0, b - 12), 255,
    Math.max(0, r - 6), Math.max(0, g - 6), Math.max(0, b - 6), 255,
    r, g, b, 255,
  ]);
  const tex = new THREE.DataTexture(data, 2, 2, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.wrapS = THREE.RepeatWrapping;
  tex.wrapT = THREE.RepeatWrapping;
  tex.needsUpdate = true;
  return tex;
}

function createCellDataTexture(): { texture: THREE.DataTexture; buffer: Uint8Array } {
  const buffer = new Uint8Array(CELL_DATA_TEX_SIZE * CELL_DATA_TEX_SIZE * 4);
  const texture = new THREE.DataTexture(
    buffer,
    CELL_DATA_TEX_SIZE,
    CELL_DATA_TEX_SIZE,
    THREE.RGBAFormat,
    THREE.UnsignedByteType,
  );
  texture.minFilter = THREE.NearestFilter;
  texture.magFilter = THREE.NearestFilter;
  texture.generateMipmaps = false;
  texture.needsUpdate = true;
  return { texture, buffer };
}

function defaultNeutralPalette(): GlobePalette {
  return {
    oceanShallowRgb: [135, 163, 173],
    oceanDeepRgb: [61, 89, 102],
    landLowRgb: [115, 138, 87],
    landMidRgb: [176, 162, 116],
    landHighRgb: [140, 114, 88],
    peakSnowRgb: [244, 239, 228],
    riverWaterRgb: [54, 108, 133],
    roadOchreRgb: [166, 89, 40],
    settlementMarkerRgb: [139, 46, 34],
    contourInkRgb: [59, 44, 29],
    compassRoseRgb: [110, 85, 57],
    unexploredVellumRgb: [230, 216, 185],
    parchmentGrainTextureUrl: "",
    hatchPatternTextureUrl: "",
    terrainBiomeTextureUrl: "",
  };
}

function encodeBiomeByte(cell: KnownCellDto): number {
  if (cell.tier === "unexplored") return 0;
  if (cell.tier === "stale") {
    return cell.elevationM < 0 ? 32 : 80;
  }
  if (cell.isOcean || cell.elevationM < 0) return 32;
  const key = (cell.biomeKey ?? "").toLowerCase();
  if (key.includes("snow") || key.includes("glacier") || key.includes("tundra")) return 224;
  if (key.includes("mountain") || key.includes("alpine") || key.includes("rock")) return 176;
  if (key.includes("arid") || key.includes("desert") || key.includes("steppe")) return 128;
  return 80;
}

function encodeElevationByte(elevM: number, isOcean: boolean): number {
  if (isOcean || elevM < 0) {
    const t = Math.max(0, Math.min(1, 1 + elevM / 5000));
    return Math.max(1, Math.min(127, Math.round(1 + t * 126)));
  }
  const t = Math.max(0, Math.min(1, elevM / 5000));
  return Math.max(128, Math.min(255, Math.round(128 + t * 127)));
}

const VERTEX_SHADER = `
in float aCellId;
in vec3 aCornerCells;
in vec3 aBarycentric;

uniform sampler2D uCellDataTex;
uniform float uTexSize;
uniform float uCartographicBlend;

out vec3 vSpherePos;
out vec3 vWorldNormal;
out vec3 vBarycentric;
out float vCellId;
out vec4 vCellData;

vec4 sampleCellData(float cid) {
  float idx = floor(cid + 0.5);
  float u = (mod(idx, uTexSize) + 0.5) / uTexSize;
  float v = (floor(idx / uTexSize) + 0.5) / uTexSize;
  return texture(uCellDataTex, vec2(u, v));
}

void main() {
  vec4 cSelf = sampleCellData(aCellId);
  vec4 c0 = sampleCellData(aCornerCells.x);
  vec4 c1 = sampleCellData(aCornerCells.y);
  vec4 c2 = sampleCellData(aCornerCells.z);

  // Strict epistemic barrier: corners bordering any Unexplored cell never lift.
  float allKnown = min(c0.a, min(c1.a, c2.a));
  float h0 = max(0.0, c0.r - 0.5) * 2.0;
  float h1 = max(0.0, c1.r - 0.5) * 2.0;
  float h2 = max(0.0, c2.r - 0.5) * 2.0;
  float avgH = (h0 + h1 + h2) / 3.0;
  float reliefScale = (1.0 - uCartographicBlend) * 0.045 * step(0.15, allKnown);

  vec3 unitP = normalize(position);
  vec3 displaced = unitP * (1.0 + avgH * reliefScale);

  vSpherePos = unitP;
  vWorldNormal = normalize(normalMatrix * unitP);
  vBarycentric = aBarycentric;
  vCellId = aCellId;
  vCellData = cSelf;

  gl_Position = projectionMatrix * modelViewMatrix * vec4(displaced, 1.0);
}
`;

const FRAGMENT_SHADER = `
precision highp float;

uniform sampler2D uCellDataTex;
uniform sampler2D uParchmentGrainTex;
uniform sampler2D uHatchPatternTex;
uniform sampler2D uTerrainBiomeTex;

uniform float uTexSize;
uniform float uCartographicBlend;
uniform vec3 uSunDir;
uniform float uHoveredCellId;
uniform float uSelectedCellId;
uniform float uWeatherOverlay;

uniform vec3 uOceanShallowRgb;
uniform vec3 uOceanDeepRgb;
uniform vec3 uLandLowRgb;
uniform vec3 uLandMidRgb;
uniform vec3 uLandHighRgb;
uniform vec3 uPeakSnowRgb;
uniform vec3 uRiverWaterRgb;
uniform vec3 uRoadOchreRgb;
uniform vec3 uSettlementMarkerRgb;
uniform vec3 uContourInkRgb;
uniform vec3 uCompassRoseRgb;
uniform vec3 uUnexploredVellumRgb;

in vec3 vSpherePos;
in vec3 vWorldNormal;
in vec3 vBarycentric;
in float vCellId;
in vec4 vCellData;

out vec4 fragColor;

vec3 sampleTriplanar(sampler2D tex, vec3 p, float scale) {
  vec3 w = pow(abs(p), vec3(4.0));
  w /= max(w.x + w.y + w.z, 0.0001);
  vec3 tx = texture(tex, p.zy * scale).rgb;
  vec3 ty = texture(tex, p.xz * scale).rgb;
  vec3 tz = texture(tex, p.xy * scale).rgb;
  return tx * w.x + ty * w.y + tz * w.z;
}

float graticuleMask(vec3 p) {
  float lat = asin(clamp(p.y, -1.0, 1.0));
  float lon = atan(p.x, p.z);
  float latFreq = 12.0 / 3.14159265;
  float lonFreq = 12.0 / 3.14159265;
  float gLat = abs(fract(lat * latFreq + 0.5) - 0.5);
  float gLon = abs(fract(lon * lonFreq + 0.5) - 0.5);
  float lineLat = 1.0 - smoothstep(0.0, 0.032, gLat);
  float lineLon = 1.0 - smoothstep(0.0, 0.032, gLon * max(0.18, cos(lat)));
  return clamp(max(lineLat, lineLon), 0.0, 1.0);
}

void main() {
  vec3 grainTex = sampleTriplanar(uParchmentGrainTex, vSpherePos, 5.5);
  vec3 hatchTex = sampleTriplanar(uHatchPatternTex, vSpherePos, 9.0);
  vec3 biomeTex = sampleTriplanar(uTerrainBiomeTex, vSpherePos, 4.0);

  float grainLum = dot(grainTex, vec3(0.333));
  float hatchInk = 1.0 - dot(hatchTex, vec3(0.333));
  float grat = graticuleMask(vSpherePos);

  float cid = floor(vCellId + 0.5);
  float edgeMin = min(vBarycentric.y, vBarycentric.z);
  float cellBorder = 1.0 - smoothstep(0.0, 0.028, vBarycentric.y + vBarycentric.z > 0.94 ? edgeMin : 1.0);
  float isHover = step(abs(cid - uHoveredCellId), 0.25);
  float isSelect = step(abs(cid - uSelectedCellId), 0.25);

  float tier = vCellData.a;
  // Unexplored cells have tier == 0.0: render pure opaque blank vellum/parchment without hidden info.
  if (tier < 0.15) {
    vec3 vellum = uUnexploredVellumRgb * (0.92 + 0.14 * grainLum);
    vellum = mix(vellum, uContourInkRgb, 0.10 * grat + 0.06 * cellBorder);
    if (isHover > 0.5 || isSelect > 0.5) {
      float highlight = max(isHover * 0.25, isSelect * 0.45);
      vellum = mix(vellum, uContourInkRgb, highlight * (0.35 + 0.65 * cellBorder));
    }
    fragColor = vec4(vellum, 1.0);
    return;
  }

  float elevNorm = vCellData.r;
  float rainNorm = vCellData.g;
  float biomeCode = vCellData.b;
  float isStale = step(tier, 0.48);

  bool isOcean = (elevNorm < 0.5) || (biomeCode < 0.20);
  vec3 reliefBase;
  if (isOcean) {
    float depthT = clamp(elevNorm * 2.0, 0.0, 1.0);
    reliefBase = mix(uOceanDeepRgb, uOceanShallowRgb, depthT);
  } else {
    float landH = clamp((elevNorm - 0.5) * 2.0, 0.0, 1.0);
    if (biomeCode > 0.80) {
      reliefBase = mix(uLandHighRgb, uPeakSnowRgb, smoothstep(0.25, 0.85, landH));
    } else if (biomeCode > 0.62) {
      reliefBase = mix(uLandMidRgb, uLandHighRgb, smoothstep(0.2, 0.9, landH));
    } else if (biomeCode > 0.42) {
      reliefBase = mix(uLandMidRgb, uLandLowRgb, 0.35);
    } else {
      reliefBase = mix(uLandLowRgb, uLandMidRgb, smoothstep(0.1, 0.65, landH));
    }
    reliefBase *= (0.86 + 0.26 * dot(biomeTex, vec3(0.333)));
  }

  float ndl = clamp(dot(normalize(vWorldNormal), normalize(uSunDir)), 0.0, 1.0);
  float hillshade = 0.52 + 0.55 * ndl;
  vec3 shadedRelief = reliefBase * hillshade;

  if (uWeatherOverlay > 0.5 && rainNorm > 0.05) {
    shadedRelief = mix(shadedRelief, uRiverWaterRgb, clamp(rainNorm * 0.55, 0.0, 0.55));
  }

  // Cartographic antique parchment map layer (uCartographicBlend near 1.0)
  vec3 mapPaper = uUnexploredVellumRgb * (0.95 + 0.10 * grainLum);
  vec3 chartLandTint = mix(uLandLowRgb, uLandMidRgb, clamp((elevNorm - 0.5) * 2.0, 0.0, 1.0));
  vec3 mapColor = isOcean
    ? mix(mapPaper, uOceanShallowRgb, 0.42)
    : mix(mapPaper, chartLandTint, 0.28);

  // Elevation contour isolines on land
  float elevSteps = max(0.0, (elevNorm - 0.5) * 14.0);
  float isolineDist = abs(fract(elevSteps) - 0.5);
  float fw = max(0.02, fwidth(elevSteps));
  float contourLine = isOcean ? 0.0 : (1.0 - smoothstep(0.03, 0.03 + fw * 1.4, isolineDist));

  // Coastline outline near elevNorm == 0.5
  float coastLine = 1.0 - smoothstep(0.015, 0.055, abs(elevNorm - 0.5));

  // Slope cross-hatching on higher relief areas
  float hatchMask = isOcean ? 0.0 : smoothstep(0.62, 0.92, elevNorm) * hatchInk * 0.7;

  mapColor = mix(mapColor, uContourInkRgb, clamp(contourLine * 0.58 + coastLine * 0.8 + hatchMask, 0.0, 0.9));
  mapColor = mix(mapColor, uCompassRoseRgb, 0.18 * grat);

  vec3 finalRgb = mix(shadedRelief, mapColor, uCartographicBlend);

  // Stale tier fogging: desaturated dusty vellum wash
  if (isStale > 0.5) {
    float luma = dot(finalRgb, vec3(0.299, 0.587, 0.114));
    vec3 faded = mix(vec3(luma), uUnexploredVellumRgb, 0.55);
    finalRgb = mix(finalRgb, faded, 0.65);
  }

  // Hover and selected cell perimeter emphasis
  if (isHover > 0.5) {
    finalRgb = mix(finalRgb, uSettlementMarkerRgb, 0.25 + 0.35 * hatchInk);
  }
  if (isSelect > 0.5) {
    finalRgb = mix(finalRgb, uCompassRoseRgb, 0.35);
  }

  fragColor = vec4(finalRgb, 1.0);
}
`;

/**
 * Builds the GPU shader pipeline for the Goldberg globe with double-buffered
 * cell attribute `DataTexture`s and external skin texture samplers (`UI-GLB-002`, `UI-GLB-004`, `UI-SKN-004`).
 */
export function createGlobeShaderController(initialPalette?: GlobePalette): GlobeShaderController {
  const palette = initialPalette ?? defaultNeutralPalette();
  const texA = createCellDataTexture();
  const texB = createCellDataTexture();
  let activeIdx = 0;

  const grainFallback = createNeutralPatternTexture(232, 224, 206);
  const hatchFallback = createNeutralPatternTexture(215, 208, 194);
  const biomeFallback = createNeutralPatternTexture(220, 220, 210);

  const loadedTextures: THREE.Texture[] = [];
  const loader = typeof window !== "undefined" ? new THREE.TextureLoader() : null;

  const uniforms = {
    uCellDataTex: { value: texA.texture },
    uTexSize: { value: CELL_DATA_TEX_SIZE },
    uCartographicBlend: { value: 0.5 },
    uSunDir: { value: new THREE.Vector3(0.45, 0.68, 0.58).normalize() },
    uHoveredCellId: { value: -1 },
    uSelectedCellId: { value: -1 },
    uWeatherOverlay: { value: 0 },
    uParchmentGrainTex: { value: grainFallback as THREE.Texture },
    uHatchPatternTex: { value: hatchFallback as THREE.Texture },
    uTerrainBiomeTex: { value: biomeFallback as THREE.Texture },
    uOceanShallowRgb: { value: rgbTupleToVec3(palette.oceanShallowRgb) },
    uOceanDeepRgb: { value: rgbTupleToVec3(palette.oceanDeepRgb) },
    uLandLowRgb: { value: rgbTupleToVec3(palette.landLowRgb) },
    uLandMidRgb: { value: rgbTupleToVec3(palette.landMidRgb) },
    uLandHighRgb: { value: rgbTupleToVec3(palette.landHighRgb) },
    uPeakSnowRgb: { value: rgbTupleToVec3(palette.peakSnowRgb) },
    uRiverWaterRgb: { value: rgbTupleToVec3(palette.riverWaterRgb) },
    uRoadOchreRgb: { value: rgbTupleToVec3(palette.roadOchreRgb) },
    uSettlementMarkerRgb: { value: rgbTupleToVec3(palette.settlementMarkerRgb) },
    uContourInkRgb: { value: rgbTupleToVec3(palette.contourInkRgb) },
    uCompassRoseRgb: { value: rgbTupleToVec3(palette.compassRoseRgb) },
    uUnexploredVellumRgb: { value: rgbTupleToVec3(palette.unexploredVellumRgb) },
  };

  const material = new THREE.ShaderMaterial({
    glslVersion: THREE.GLSL3,
    uniforms,
    vertexShader: VERTEX_SHADER,
    fragmentShader: FRAGMENT_SHADER,
  });

  const applyTexUrl = (url: string, targetUniform: { value: THREE.Texture }) => {
    if (!loader || !url) return;
    loader.load(
      url,
      (loaded) => {
        loaded.wrapS = THREE.RepeatWrapping;
        loaded.wrapT = THREE.RepeatWrapping;
        loaded.needsUpdate = true;
        loadedTextures.push(loaded);
        targetUniform.value = loaded;
      },
      undefined,
      () => {
        // Retain fallback procedural neutral texture cleanly on load errors.
      },
    );
  };

  const setPalette = (next: GlobePalette) => {
    uniforms.uOceanShallowRgb.value.copy(rgbTupleToVec3(next.oceanShallowRgb));
    uniforms.uOceanDeepRgb.value.copy(rgbTupleToVec3(next.oceanDeepRgb));
    uniforms.uLandLowRgb.value.copy(rgbTupleToVec3(next.landLowRgb));
    uniforms.uLandMidRgb.value.copy(rgbTupleToVec3(next.landMidRgb));
    uniforms.uLandHighRgb.value.copy(rgbTupleToVec3(next.landHighRgb));
    uniforms.uPeakSnowRgb.value.copy(rgbTupleToVec3(next.peakSnowRgb));
    uniforms.uRiverWaterRgb.value.copy(rgbTupleToVec3(next.riverWaterRgb));
    uniforms.uRoadOchreRgb.value.copy(rgbTupleToVec3(next.roadOchreRgb));
    uniforms.uSettlementMarkerRgb.value.copy(rgbTupleToVec3(next.settlementMarkerRgb));
    uniforms.uContourInkRgb.value.copy(rgbTupleToVec3(next.contourInkRgb));
    uniforms.uCompassRoseRgb.value.copy(rgbTupleToVec3(next.compassRoseRgb));
    uniforms.uUnexploredVellumRgb.value.copy(rgbTupleToVec3(next.unexploredVellumRgb));
    applyTexUrl(next.parchmentGrainTextureUrl, uniforms.uParchmentGrainTex);
    applyTexUrl(next.hatchPatternTextureUrl, uniforms.uHatchPatternTex);
    applyTexUrl(next.terrainBiomeTextureUrl, uniforms.uTerrainBiomeTex);
  };

  setPalette(palette);

  return {
    material,
    activeTextureIndex: () => activeIdx,
    uploadSnapshot(
      snapshot: DiegeticSnapshot,
      selectedCellId: number | null,
      hoveredCellId: number | null,
    ) {
      const nextIdx = 1 - activeIdx;
      const target = nextIdx === 0 ? texA : texB;
      const buf = target.buffer;
      buf.fill(0);

      const weatherByCell = new Map<number, ObservedWeatherDto>();
      for (const w of snapshot.observedWeather) {
        weatherByCell.set(w.cellId, w);
      }

      for (const cell of snapshot.knownCells) {
        if (cell.cellId < 0 || cell.cellId >= CELL_DATA_TEX_SIZE * CELL_DATA_TEX_SIZE) continue;
        if (cell.tier === "unexplored") {
          // Strict zero fill on Unexplored slots (`UI-GLB-005`).
          continue;
        }
        const offset = cell.cellId * 4;
        const isOcean = cell.tier === "surveyed" ? Boolean(cell.isOcean || cell.elevationM < 0) : cell.elevationM < 0;
        buf[offset] = encodeElevationByte(cell.elevationM, isOcean);

        const weather = weatherByCell.get(cell.cellId);
        const precip = weather ? Math.max(0, Math.min(1, weather.precipMmHr / 25)) : 0;
        buf[offset + 1] = cell.tier === "surveyed" ? Math.round(precip * 255) : 0;
        buf[offset + 2] = encodeBiomeByte(cell);

        let alpha = cell.tier === "stale" ? 85 : 170;
        if (cell.cellId === selectedCellId || cell.cellId === hoveredCellId) {
          alpha = 255;
        } else if (cell.tier === "surveyed" && precip > 0.15) {
          alpha = 212;
        }
        buf[offset + 3] = alpha;
      }

      target.texture.needsUpdate = true;
      activeIdx = nextIdx;
      uniforms.uCellDataTex.value = target.texture;
      uniforms.uSelectedCellId.value = selectedCellId ?? -1;
      uniforms.uHoveredCellId.value = hoveredCellId ?? -1;
    },
    setPalette,
    setCartographicState(
      blend01: number,
      sunDir: THREE.Vector3,
      hoveredCellId: number | null,
      selectedCellId: number | null,
      activeWeatherOverlay: boolean,
    ) {
      uniforms.uCartographicBlend.value = Math.max(0, Math.min(1, blend01));
      uniforms.uSunDir.value.copy(sunDir).normalize();
      uniforms.uHoveredCellId.value = hoveredCellId ?? -1;
      uniforms.uSelectedCellId.value = selectedCellId ?? -1;
      uniforms.uWeatherOverlay.value = activeWeatherOverlay ? 1 : 0;
    },
    dispose() {
      texA.texture.dispose();
      texB.texture.dispose();
      grainFallback.dispose();
      hatchFallback.dispose();
      biomeFallback.dispose();
      for (const lt of loadedTextures) lt.dispose();
      material.dispose();
    },
  };
}
