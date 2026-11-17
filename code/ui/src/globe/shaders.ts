import * as THREE from "three";
import {
  DiegeticSnapshot,
  GlobePalette,
  GlobeProjectionMode,
  KnownCellDto,
  ObservedWeatherDto,
  RgbTuple,
  Vec3,
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
    hoveredUnitXyz?: readonly [number, number, number] | null,
    selectedUnitXyz?: readonly [number, number, number] | null,
  ): void;
  setAltitudeNorm?(altitudeNorm01: number): void;
  setParchmentMode?(enabled: boolean): void;
  setProjectionState?(
    mode: GlobeProjectionMode,
    morph01: number,
    focusUnit: Vec3,
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

function inferTempFallback(cell: Extract<KnownCellDto, { tier: "stale" | "surveyed" }>): number {
  if (cell.meanAnnualTempC !== undefined) return cell.meanAnnualTempC;
  const key = (cell.biomeKey ?? "").toLowerCase();
  if (key.includes("glacier") || key.includes("snow")) return -10;
  if (key.includes("tundra")) return -4;
  if (key.includes("boreal") || key.includes("taiga")) return 2;
  if (key.includes("desert") || key.includes("arid")) return 25;
  if (key.includes("tropical")) return 26;
  return 15 - Math.max(0, cell.elevationM) * 0.0065;
}

function inferAridityFallback(cell: Extract<KnownCellDto, { tier: "stale" | "surveyed" }>): number {
  if (cell.aridityIndex01 !== undefined) return cell.aridityIndex01;
  const key = (cell.biomeKey ?? "").toLowerCase();
  if (key.includes("desert") || key.includes("arid")) return 0.16;
  if (key.includes("steppe") || key.includes("savanna")) return 0.48;
  if (key.includes("rainforest")) return 1.5;
  return 0.95;
}

function inferFertilityFallback(cell: Extract<KnownCellDto, { tier: "stale" | "surveyed" }>): number {
  if (cell.soilFertility01 !== undefined) return cell.soilFertility01;
  if (cell.isOcean || cell.elevationM < 0) return 0;
  const key = (cell.biomeKey ?? "").toLowerCase();
  if (key.includes("desert") || key.includes("glacier") || key.includes("rock")) return 0.15;
  if (key.includes("temperate") || key.includes("grass") || key.includes("alluvial")) return 0.78;
  return 0.52;
}

function encodeBiophysicalBytes(
  cell: KnownCellDto,
  weather: ObservedWeatherDto | undefined,
): readonly [number, number, number, number] {
  if (cell.tier === "unexplored" || cell.tier === "rumored") {
    return [0, 0, 0, 0];
  }
  const liveTemp = weather ? weather.tempC : inferTempFallback(cell);
  const tempByte = Math.max(0, Math.min(255, Math.round(((liveTemp + 30) / 65) * 255)));

  const aridity = inferAridityFallback(cell);
  const aridityByte = Math.max(0, Math.min(255, Math.round((aridity / 1.8) * 255)));

  const fertNibble = Math.max(0, Math.min(15, Math.round(inferFertilityFallback(cell) * 15)));
  const strataNibble = Math.max(
    0,
    Math.min(15, cell.rockStrataCode ?? (cell.isOcean || cell.elevationM < 0 ? 0 : 4)),
  );
  const fertStrataByte = (fertNibble << 4) | strataNibble;

  const strahler3 = Math.max(
    0,
    Math.min(7, cell.strahlerOrder ?? ((cell.riverFlowNorm01 ?? 0) > 0.45 ? 2 : 1)),
  );
  const snow5 = Math.max(0, Math.min(31, Math.round((weather?.snowDepthM ?? 0) * 8)));
  const hydroSnowByte = (strahler3 << 5) | snow5;

  return [tempByte, aridityByte, fertStrataByte, hydroSnowByte];
}

const VERTEX_SHADER = `
in float aCellId;
in vec3 aCornerCells;
in vec3 aBarycentric;
in vec3 aTriCenter;
in float aSkirtMask;

uniform sampler2D uCellDataTex;
uniform sampler2D uBiophysicalTex;
uniform float uTexSize;
uniform float uCartographicBlend;
uniform float uCameraAltitudeNorm;
uniform float uProjectionMode;
uniform float uProjectionMorph;
uniform vec3 uFocusUnit;

out vec3 vSpherePos;
out vec3 vWorldPos;
out vec3 vViewNormal;
out vec3 vWorldSurfNormal;
out vec3 vBarycentric;
flat out vec3 vCornerCells;
out float vCellId;
out vec4 vCellData;
out vec2 vFlatCoord;
out float vSeamDiscard;
out float vReliefHeight;

const float PI = 3.141592653589793;
const float EQUAL_AREA_SCALE = 0.72;
const float MERCATOR_SCALE = 1.35 / 3.141592653589793;
const float MERCATOR_MAX_LAT = 1.4835298641951802;

vec4 sampleCellData(float cid) {
  float idx = floor(cid + 0.5);
  float u = (mod(idx, uTexSize) + 0.5) / uTexSize;
  float v = (floor(idx / uTexSize) + 0.5) / uTexSize;
  return texture(uCellDataTex, vec2(u, v));
}

vec4 sampleBiophysicalData(float cid) {
  float idx = floor(cid + 0.5);
  float u = (mod(idx, uTexSize) + 0.5) / uTexSize;
  float v = (floor(idx / uTexSize) + 0.5) / uTexSize;
  return texture(uBiophysicalTex, vec2(u, v));
}

float hash31(vec3 p) {
  p = fract(p * vec3(443.897, 441.423, 437.195));
  p += dot(p, p.yzx + 19.19);
  return fract((p.x + p.y) * p.z);
}

float vnoise3(vec3 p) {
  vec3 i = floor(p);
  vec3 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  float n000 = hash31(i + vec3(0.0, 0.0, 0.0));
  float n100 = hash31(i + vec3(1.0, 0.0, 0.0));
  float n010 = hash31(i + vec3(0.0, 1.0, 0.0));
  float n110 = hash31(i + vec3(1.0, 1.0, 0.0));
  float n001 = hash31(i + vec3(0.0, 0.0, 1.0));
  float n101 = hash31(i + vec3(1.0, 0.0, 1.0));
  float n011 = hash31(i + vec3(0.0, 1.0, 1.0));
  float n111 = hash31(i + vec3(1.0, 1.0, 1.0));
  float nx00 = mix(n000, n100, f.x);
  float nx10 = mix(n010, n110, f.x);
  float nx01 = mix(n001, n101, f.x);
  float nx11 = mix(n011, n111, f.x);
  return mix(mix(nx00, nx10, f.y), mix(nx01, nx11, f.y), f.z);
}

float fbm4(vec3 p) {
  float sum = 0.0;
  float amp = 0.5;
  vec3 shift = vec3(13.7, -7.3, 21.1);
  for (int i = 0; i < 4; i++) {
    sum += amp * vnoise3(p);
    p = p * 2.07 + shift;
    amp *= 0.5;
  }
  return sum;
}

float ridgedFbm3(vec3 p) {
  float sum = 0.0;
  float amp = 0.55;
  float norm = 0.0;
  for (int i = 0; i < 3; i++) {
    float r = 1.0 - abs(vnoise3(p) * 2.0 - 1.0);
    sum += (r * r) * amp;
    norm += amp;
    p = p * 2.12 + vec3(7.1, 3.9, -11.3);
    amp *= 0.52;
  }
  return sum / max(norm, 0.001);
}

float decodeElevMeters(float elevNorm) {
  if (elevNorm < 0.5) {
    return (elevNorm / 0.5 - 1.0) * 5000.0;
  }
  return ((elevNorm - 0.5) / 0.5) * 5000.0;
}

float evalProceduralRelief(
  vec3 unitP,
  float baseElevM,
  float coastBand,
  float strahler,
  float kSum
) {
  vec3 warpVec = vec3(
    fbm4(unitP * 9.0 + vec3(1.2, 4.7, -2.3)),
    fbm4(unitP * 9.0 + vec3(-3.8, 0.6, 5.1)),
    fbm4(unitP * 9.0 + vec3(2.9, -4.1, 1.4))
  ) - 0.5;
  float absLat = abs(asin(clamp(unitP.y, -1.0, 1.0)));
  float coastMacro = (fbm4(unitP * 16.0 + warpVec * 1.45) - 0.48) * 320.0;
  float coastDetail = (fbm4(unitP * 44.0 + warpVec * 2.1) - 0.50) * 145.0;
  float fjordMask = smoothstep(0.68, 1.15, absLat);
  float fjordCuts = (ridgedFbm3(unitP * 34.0 + warpVec * 1.8) - 0.46) * 290.0 * fjordMask;
  float microRidgeM = (ridgedFbm3(unitP * 28.0 + warpVec * 0.8) - 0.35)
    * smoothstep(80.0, 1400.0, max(0.0, baseElevM)) * 420.0;

  float effectiveElevM = baseElevM + (coastMacro + coastDetail - fjordCuts) * coastBand + microRidgeM;
  float posElev = max(0.0, effectiveElevM);
  float landMask = smoothstep(0.0, 6.0, posElev);
  float coastalBluff = smoothstep(0.0, 65.0, posElev) * 0.0052;
  float hypsoBase = pow(clamp(posElev / 4200.0, 0.0, 1.0), 0.65) * 0.060;
  float rollingHills = (fbm4(unitP * 34.0 + vec3(3.1, -1.7, 5.3)) - 0.42)
    * 0.0054 * smoothstep(2.0, 35.0, posElev);
  float valleyCut = 0.0036
    * smoothstep(1.25, 3.8, strahler)
    * smoothstep(8.0, 85.0, posElev)
    * (1.0 - smoothstep(450.0, 1800.0, posElev));
  float alpineCrags = ridgedFbm3(unitP * 26.0 + vec3(-2.4, 5.1, 1.7))
    * 0.019 * smoothstep(180.0, 1700.0, posElev);

  float rawLandRelief = max(0.0003 * landMask, coastalBluff + hypsoBase + rollingHills + alpineCrags - valleyCut) * landMask;
  return rawLandRelief * smoothstep(0.42, 0.82, kSum);
}

vec2 projectUnitFlat(vec3 u, vec3 focusNorm, out float validOut) {
  validOut = 1.0;
  if (uProjectionMode < 1.5) {
    vec3 center = normalize(focusNorm);
    vec3 upRef = abs(center.y) < 0.99 ? vec3(0.0, 1.0, 0.0) : vec3(0.0, 0.0, -1.0);
    vec3 east = normalize(cross(upRef, center));
    vec3 north = normalize(cross(center, east));
    float tx = dot(u, east);
    float ty = dot(u, north);
    float tz = dot(u, center);
    if (1.0 + tz <= 0.0005) {
      validOut = 0.0;
      return vec2(EQUAL_AREA_SCALE * 2.0, 0.0);
    }
    float k = sqrt(2.0 / (1.0 + tz));
    return vec2(EQUAL_AREA_SCALE * k * tx, EQUAL_AREA_SCALE * k * ty);
  }
  float lon0 = atan(focusNorm.x, focusNorm.z);
  float lon = atan(u.x, u.z);
  float dLon = lon - lon0;
  dLon -= 2.0 * PI * floor((dLon + PI) / (2.0 * PI));
  float rawLat = asin(clamp(u.y, -1.0, 1.0));
  float clampedLat = clamp(rawLat, -MERCATOR_MAX_LAT, MERCATOR_MAX_LAT);
  if (abs(rawLat) > MERCATOR_MAX_LAT + 0.01) {
    validOut = 0.0;
  }
  float my = MERCATOR_SCALE * log(tan(PI * 0.25 + clampedLat * 0.5));
  return vec2(MERCATOR_SCALE * dLon, my);
}

void main() {
  vec4 d0 = sampleCellData(aCornerCells.x);
  vec4 d1 = sampleCellData(aCornerCells.y);
  vec4 d2 = sampleCellData(aCornerCells.z);
  vec4 cSelf = sampleCellData(aCellId);

  vec3 sb = smoothstep(0.0, 1.0, clamp(aBarycentric, 0.0, 1.0));
  sb /= max(sb.x + sb.y + sb.z, 0.0001);

  float k0 = step(0.08, d0.a) * sb.x;
  float k1 = step(0.08, d1.a) * sb.y;
  float k2 = step(0.08, d2.a) * sb.z;
  float kSum = k0 + k1 + k2;

  vec3 unitP = normalize(position);
  float reliefHeight = 0.0;
  float baseElevM = -5000.0;
  float coastBand = 0.0;
  float strahler = 1.0;

  if (kSum >= 0.35) {
    float w0 = k0 / kSum;
    float w1 = k1 / kSum;
    float w2 = k2 / kSum;
    float elevNorm = d0.r * w0 + d1.r * w1 + d2.r * w2;
    baseElevM = decodeElevMeters(elevNorm);

    float isLand0 = step(0.499, d0.r);
    float isLand1 = step(0.499, d1.r);
    float isLand2 = step(0.499, d2.r);
    float landFrac = isLand0 * w0 + isLand1 * w1 + isLand2 * w2;
    float coastTransition = smoothstep(0.05, 0.32, landFrac) * smoothstep(0.05, 0.32, 1.0 - landFrac);
    coastBand = (1.0 - smoothstep(40.0, 520.0, abs(baseElevM))) * coastTransition;

    vec4 bio0 = sampleBiophysicalData(aCornerCells.x);
    vec4 bio1 = sampleBiophysicalData(aCornerCells.y);
    vec4 bio2 = sampleBiophysicalData(aCornerCells.z);
    float b0Mask = step(0.24, d0.a) * w0;
    float b1Mask = step(0.24, d1.a) * w1;
    float b2Mask = step(0.24, d2.a) * w2;
    float bSum = b0Mask + b1Mask + b2Mask;
    if (bSum >= 0.01) {
      float bw0 = b0Mask / bSum;
      float bw1 = b1Mask / bSum;
      float bw2 = b2Mask / bSum;
      float fa0 = floor(bio0.a * 255.0 + 0.5);
      float fa1 = floor(bio1.a * 255.0 + 0.5);
      float fa2 = floor(bio2.a * 255.0 + 0.5);
      strahler = floor(fa0 / 32.0) * bw0 + floor(fa1 / 32.0) * bw1 + floor(fa2 / 32.0) * bw2;
    }

    reliefHeight = evalProceduralRelief(unitP, baseElevM, coastBand, strahler, kSum);
  }

  // Strict epistemic barrier: Unexplored vertices remain smoothly spherical and never lift.
  float skirtDrop = aSkirtMask * 0.0065;
  vec3 displacedSphere = unitP * (1.0 + reliefHeight - skirtDrop);
  vec3 sphereSurfNormal = unitP;

  vec3 flatPos = displacedSphere;
  vec3 flatSurfNormal = vec3(0.0, 0.0, 1.0);
  float flatValid = 1.0;
  vec2 flatXY = vec2(0.0);
  float seamDiscard = 0.0;

  if (uProjectionMode > 0.5 && uProjectionMorph > 0.001) {
    vec3 focusNorm = length(uFocusUnit) > 0.5 ? normalize(uFocusUnit) : vec3(0.0, 0.0, 1.0);
    flatXY = projectUnitFlat(unitP, focusNorm, flatValid);
    if (length(aTriCenter) > 0.5) {
      float triValid = 1.0;
      vec2 triXY = projectUnitFlat(normalize(aTriCenter), focusNorm, triValid);
      if (flatValid < 0.5 || triValid < 0.5 || distance(flatXY, triXY) > 0.72) {
        seamDiscard = 1.0;
      }
    } else if (flatValid < 0.5) {
      seamDiscard = 1.0;
    }
    flatPos = vec3(flatXY.x, flatXY.y, reliefHeight * 0.65 - skirtDrop);
  }

  float morph = clamp(uProjectionMorph, 0.0, 1.0);
  vec3 worldPos = mix(displacedSphere, flatPos, morph);
  vec3 outNormal = normalize(mix(sphereSurfNormal, flatSurfNormal, morph));

  vSpherePos = unitP;
  vWorldPos = worldPos;
  vViewNormal = normalize(normalMatrix * outNormal);
  vWorldSurfNormal = outNormal;
  vBarycentric = aBarycentric;
  vCornerCells = aCornerCells;
  vCellId = aCellId;
  vCellData = cSelf;
  vFlatCoord = flatXY;
  vSeamDiscard = seamDiscard;
  vReliefHeight = reliefHeight;

  gl_Position = projectionMatrix * modelViewMatrix * vec4(worldPos, 1.0);
}
`;

const FRAGMENT_SHADER = `
precision highp float;

uniform sampler2D uCellDataTex;
uniform sampler2D uBiophysicalTex;
uniform sampler2D uParchmentGrainTex;
uniform sampler2D uHatchPatternTex;
uniform sampler2D uTerrainBiomeTex;

uniform float uTexSize;
uniform float uCartographicBlend;
uniform float uCameraAltitudeNorm;
uniform float uParchmentMode;
uniform float uProjectionMorph;
uniform vec3 uSunDir;
uniform float uHoveredCellId;
uniform float uSelectedCellId;
uniform vec3 uHoveredUnitXyz;
uniform vec3 uSelectedUnitXyz;
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
in vec3 vWorldPos;
in vec3 vViewNormal;
in vec3 vWorldSurfNormal;
in vec3 vBarycentric;
flat in vec3 vCornerCells;
in float vCellId;
in vec4 vCellData;
in vec2 vFlatCoord;
in float vSeamDiscard;
in float vReliefHeight;

out vec4 fragColor;

const float PI = 3.141592653589793;

vec4 sampleCellData(float cid) {
  float idx = floor(cid + 0.5);
  float u = (mod(idx, uTexSize) + 0.5) / uTexSize;
  float v = (floor(idx / uTexSize) + 0.5) / uTexSize;
  return texture(uCellDataTex, vec2(u, v));
}

vec4 sampleBiophysicalData(float cid) {
  float idx = floor(cid + 0.5);
  float u = (mod(idx, uTexSize) + 0.5) / uTexSize;
  float v = (floor(idx / uTexSize) + 0.5) / uTexSize;
  return texture(uBiophysicalTex, vec2(u, v));
}

float hash31(vec3 p) {
  p = fract(p * vec3(443.897, 441.423, 437.195));
  p += dot(p, p.yzx + 19.19);
  return fract((p.x + p.y) * p.z);
}

float vnoise3(vec3 p) {
  vec3 i = floor(p);
  vec3 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  float n000 = hash31(i + vec3(0.0, 0.0, 0.0));
  float n100 = hash31(i + vec3(1.0, 0.0, 0.0));
  float n010 = hash31(i + vec3(0.0, 1.0, 0.0));
  float n110 = hash31(i + vec3(1.0, 1.0, 0.0));
  float n001 = hash31(i + vec3(0.0, 0.0, 1.0));
  float n101 = hash31(i + vec3(1.0, 0.0, 1.0));
  float n011 = hash31(i + vec3(0.0, 1.0, 1.0));
  float n111 = hash31(i + vec3(1.0, 1.0, 1.0));
  float nx00 = mix(n000, n100, f.x);
  float nx10 = mix(n010, n110, f.x);
  float nx01 = mix(n001, n101, f.x);
  float nx11 = mix(n011, n111, f.x);
  return mix(mix(nx00, nx10, f.y), mix(nx01, nx11, f.y), f.z);
}

float fbm4(vec3 p) {
  float sum = 0.0;
  float amp = 0.5;
  vec3 shift = vec3(13.7, -7.3, 21.1);
  for (int i = 0; i < 4; i++) {
    sum += amp * vnoise3(p);
    p = p * 2.07 + shift;
    amp *= 0.5;
  }
  return sum;
}

float ridgedFbm3(vec3 p) {
  float sum = 0.0;
  float amp = 0.55;
  float norm = 0.0;
  for (int i = 0; i < 3; i++) {
    float r = 1.0 - abs(vnoise3(p) * 2.0 - 1.0);
    sum += (r * r) * amp;
    norm += amp;
    p = p * 2.12 + vec3(7.1, 3.9, -11.3);
    amp *= 0.52;
  }
  return sum / max(norm, 0.001);
}

vec3 fieldParcelPattern(vec3 p) {
  vec3 q = p * 86.0;
  vec3 cellId = floor(q);
  vec3 loc = fract(q);
  float angleHash = hash31(cellId) * 6.28318;
  float ca = cos(angleHash);
  float sa = sin(angleHash);
  vec2 rot = vec2(loc.x * ca - loc.z * sa, loc.x * sa + loc.z * ca);
  vec2 tile = fract(rot * 2.6 + vec2(hash31(cellId + 3.0), hash31(cellId + 7.0)));
  float edgeMask = smoothstep(0.0, 0.085, tile.x) * smoothstep(0.0, 0.085, tile.y)
    * smoothstep(0.0, 0.085, 1.0 - tile.x) * smoothstep(0.0, 0.085, 1.0 - tile.y);
  float parcelTone = hash31(cellId + floor(rot * 2.6).x * 11.0 + floor(rot * 2.6).y * 23.0);
  float furrow = 0.5 + 0.5 * sin(rot.y * 125.0 + parcelTone * 6.28);
  return vec3(mix(0.32, 0.65 + 0.35 * parcelTone, edgeMask), 1.0 - edgeMask, furrow);
}

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
  float latFreq = 12.0 / PI;
  float lonFreq = 12.0 / PI;
  float gLat = abs(fract(lat * latFreq + 0.5) - 0.5);
  float gLon = abs(fract(lon * lonFreq + 0.5) - 0.5);
  float lineLat = 1.0 - smoothstep(0.0, 0.028, gLat);
  float lineLon = 1.0 - smoothstep(0.0, 0.028, gLon * max(0.18, cos(lat)));
  return clamp(max(lineLat, lineLon), 0.0, 1.0);
}

float decodeElevMeters(float elevNorm) {
  if (elevNorm < 0.5) {
    return (elevNorm / 0.5 - 1.0) * 5000.0;
  }
  return ((elevNorm - 0.5) / 0.5) * 5000.0;
}

void main() {
  if (vSeamDiscard > 0.01) {
    discard;
  }

  vec3 unitS = normalize(vSpherePos);
  vec3 sunDir = normalize(uSunDir);
  vec3 viewNorm = normalize(vViewNormal);
  vec3 worldNorm = normalize(vWorldSurfNormal);
  float grat = graticuleMask(unitS);

  vec3 grainTex = sampleTriplanar(uParchmentGrainTex, unitS, 5.5);
  vec3 hatchTex = sampleTriplanar(uHatchPatternTex, unitS, 9.0);
  float grainLum = dot(grainTex, vec3(0.333));
  float hatchInk = 1.0 - dot(hatchTex, vec3(0.333));

  // Smoothstepped barycentric weights remove linear triangle & hexagonal Voronoi facets.
  vec3 sb = smoothstep(0.0, 1.0, clamp(vBarycentric, 0.0, 1.0));
  float sbSum = max(sb.x + sb.y + sb.z, 0.0001);
  sb /= sbSum;

  vec4 d0 = sampleCellData(vCornerCells.x);
  vec4 d1 = sampleCellData(vCornerCells.y);
  vec4 d2 = sampleCellData(vCornerCells.z);

  float k0 = step(0.08, d0.a) * sb.x;
  float k1 = step(0.08, d1.a) * sb.y;
  float k2 = step(0.08, d2.a) * sb.z;
  float kSum = k0 + k1 + k2;

  // Reticle rings around continuously picked coordinate.
  float cid = floor(vCellId + 0.5);
  bool hasHoverVec = length(uHoveredUnitXyz) > 0.5;
  bool hasSelectVec = length(uSelectedUnitXyz) > 0.5;
  float hoverAng = hasHoverVec ? acos(clamp(dot(unitS, normalize(uHoveredUnitXyz)), -1.0, 1.0)) : 9.0;
  float selectAng = hasSelectVec ? acos(clamp(dot(unitS, normalize(uSelectedUnitXyz)), -1.0, 1.0)) : 9.0;
  float reticleScale = mix(1.0, 0.34, smoothstep(0.45, 0.02, uCameraAltitudeNorm));
  float hoverRing = hasHoverVec
    ? (1.0 - smoothstep(0.004 * reticleScale, 0.010 * reticleScale, abs(hoverAng - 0.018 * reticleScale))) * step(hoverAng, 0.04 * reticleScale)
    : step(abs(cid - uHoveredCellId), 0.25);
  float selectRing = hasSelectVec
    ? (1.0 - smoothstep(0.005 * reticleScale, 0.012 * reticleScale, abs(selectAng - 0.024 * reticleScale))) * step(selectAng, 0.05 * reticleScale)
    : step(abs(cid - uSelectedCellId), 0.25);

  float fresnelRim = pow(1.0 - clamp(abs(dot(viewNorm, vec3(0.0, 0.0, 1.0))), 0.0, 1.0), 3.2);
  float sunLimb = clamp(dot(unitS, sunDir) * 0.55 + 0.45, 0.15, 1.0);
  vec3 twilightBase = mix(
    vec3(0.038, 0.050, 0.072),
    vec3(0.072, 0.095, 0.132),
    clamp(dot(unitS, sunDir) * 0.5 + 0.5, 0.0, 1.0)
  );
  twilightBase = mix(twilightBase, uUnexploredVellumRgb * 0.18, 0.24);
  twilightBase = mix(twilightBase, mix(vec3(0.14, 0.19, 0.26), uCompassRoseRgb, 0.55), 0.22 * grat);
  twilightBase += vec3(0.16, 0.30, 0.52) * fresnelRim * sunLimb * (1.0 - uProjectionMorph) * 0.42;

  vec3 vellumBase = uUnexploredVellumRgb * (0.92 + 0.14 * grainLum);
  vellumBase = mix(vellumBase, uContourInkRgb, 0.10 * grat);
  vec3 unmappedColor = mix(twilightBase, vellumBase, uParchmentMode);

  // Strict epistemic barrier: purely unexplored regions never leak terrain data.
  if (kSum < 0.35) {
    if (hoverRing > 0.05 || selectRing > 0.05) {
      vec3 ringCol = mix(vec3(0.86, 0.72, 0.42), uContourInkRgb, uParchmentMode);
      unmappedColor = mix(unmappedColor, ringCol, max(hoverRing * 0.55, selectRing * 0.75));
    }
    fragColor = vec4(unmappedColor, 1.0);
    return;
  }

  // Organic domain-warped cloud-bank feather strictly inside the revealed perimeter.
  float horizonNoise = fbm4(unitS * 14.5 + vec3(2.1, -3.4, 5.2));
  float featheredK = kSum + (horizonNoise - 0.5) * 0.24;
  float fogReveal = smoothstep(0.46, 0.84, featheredK);
  float cloudMask = smoothstep(0.44, 0.65, featheredK) * (1.0 - smoothstep(0.68, 0.90, featheredK));

  float w0 = k0 / kSum;
  float w1 = k1 / kSum;
  float w2 = k2 / kSum;

  float elevNorm = d0.r * w0 + d1.r * w1 + d2.r * w2;
  float rainNorm = d0.g * w0 + d1.g * w1 + d2.g * w2;
  float tier = d0.a * w0 + d1.a * w1 + d2.a * w2;

  float isRumored = step(0.08, tier) * step(tier, 0.25);
  float isStale = step(0.25, tier) * step(tier, 0.48);
  float isDatedSurvey = step(0.48, tier) * step(tier, 0.71);

  // Sample & smoothly interpolate biophysical descriptors only from stale/surveyed corners.
  vec4 bio0 = sampleBiophysicalData(vCornerCells.x);
  vec4 bio1 = sampleBiophysicalData(vCornerCells.y);
  vec4 bio2 = sampleBiophysicalData(vCornerCells.z);
  float b0Mask = step(0.24, d0.a) * w0;
  float b1Mask = step(0.24, d1.a) * w1;
  float b2Mask = step(0.24, d2.a) * w2;
  float bSum = max(b0Mask + b1Mask + b2Mask, 0.0001);
  float bw0 = b0Mask / bSum;
  float bw1 = b1Mask / bSum;
  float bw2 = b2Mask / bSum;

  float tempC = (bio0.r * bw0 + bio1.r * bw1 + bio2.r * bw2) * 65.0 - 30.0;
  float aridity = (bio0.g * bw0 + bio1.g * bw1 + bio2.g * bw2) * 1.8;

  float fb0 = floor(bio0.b * 255.0 + 0.5);
  float fb1 = floor(bio1.b * 255.0 + 0.5);
  float fb2 = floor(bio2.b * 255.0 + 0.5);
  float fertility = (floor(fb0 / 16.0) * bw0 + floor(fb1 / 16.0) * bw1 + floor(fb2 / 16.0) * bw2) / 15.0;
  float strataRough = (mod(fb0, 16.0) * bw0 + mod(fb1, 16.0) * bw1 + mod(fb2, 16.0) * bw2);

  float fa0 = floor(bio0.a * 255.0 + 0.5);
  float fa1 = floor(bio1.a * 255.0 + 0.5);
  float fa2 = floor(bio2.a * 255.0 + 0.5);
  float strahler = floor(fa0 / 32.0) * bw0 + floor(fa1 / 32.0) * bw1 + floor(fa2 / 32.0) * bw2;
  float snowPackM = (mod(fa0, 32.0) * bw0 + mod(fa1, 32.0) * bw1 + mod(fa2, 32.0) * bw2) / 8.0;

  if (b0Mask + b1Mask + b2Mask < 0.01) {
    tempC = 14.0 - max(0.0, decodeElevMeters(elevNorm)) * 0.0065;
    aridity = 0.85;
    fertility = 0.50;
    strataRough = 4.0;
    strahler = 1.0;
    snowPackM = 0.0;
  }

  // Organic domain-warped coastal & high-latitude fjord displacement prior to sea-level test.
  float absLat = abs(asin(clamp(unitS.y, -1.0, 1.0)));
  float baseElevM = decodeElevMeters(elevNorm);
  vec3 warpVec = vec3(
    fbm4(unitS * 9.0 + vec3(1.2, 4.7, -2.3)),
    fbm4(unitS * 9.0 + vec3(-3.8, 0.6, 5.1)),
    fbm4(unitS * 9.0 + vec3(2.9, -4.1, 1.4))
  ) - 0.5;
  float isLand0 = step(0.499, d0.r);
  float isLand1 = step(0.499, d1.r);
  float isLand2 = step(0.499, d2.r);
  float landFrac = isLand0 * w0 + isLand1 * w1 + isLand2 * w2;
  float coastTransition = smoothstep(0.05, 0.32, landFrac) * smoothstep(0.05, 0.32, 1.0 - landFrac);
  float coastBand = (1.0 - smoothstep(40.0, 520.0, abs(baseElevM))) * coastTransition;
  float coastMacro = (fbm4(unitS * 16.0 + warpVec * 1.45) - 0.48) * 320.0;
  float coastDetail = (fbm4(unitS * 44.0 + warpVec * 2.1) - 0.50) * 145.0;
  float fjordMask = smoothstep(0.68, 1.15, absLat);
  float fjordCuts = (ridgedFbm3(unitS * 34.0 + warpVec * 1.8) - 0.46) * 290.0 * fjordMask;
  float microRidgeM = (ridgedFbm3(unitS * 28.0 + warpVec * 0.8) - 0.35) * smoothstep(80.0, 1400.0, max(0.0, baseElevM)) * 420.0;

  float effectiveElevM = baseElevM + (coastMacro + coastDetail - fjordCuts) * coastBand + microRidgeM;
  bool isOceanPixel = effectiveElevM < 0.0;

  // World-space surface basis and screen-space elevation derivative projected into world tangent space
  // so 3D mountain & hillside lighting remains locked to geography at all pitch angles (0..45 deg).
  vec3 sphereRef = abs(unitS.y) < 0.99 ? vec3(0.0, 1.0, 0.0) : vec3(0.0, 0.0, -1.0);
  vec3 sEast = normalize(cross(sphereRef, unitS));
  vec3 sNorth = normalize(cross(unitS, sEast));
  vec3 eastT = normalize(mix(sEast, vec3(1.0, 0.0, 0.0), uProjectionMorph));
  vec3 northT = normalize(mix(sNorth, vec3(0.0, 1.0, 0.0), uProjectionMorph));

  float closeDetail01 = smoothstep(0.38, 0.03, uCameraAltitudeNorm);
  float quadValid = smoothstep(0.42, 0.70, kSum);
  float dxE = dFdx(effectiveElevM) * quadValid;
  float dyE = dFdy(effectiveElevM) * quadValid;
  float dxR = dFdx(vReliefHeight) * quadValid;
  float dyR = dFdy(vReliefHeight) * quadValid;
  vec3 dpdx = dFdx(vWorldPos);
  vec3 dpdy = dFdy(vWorldPos);
  float a11 = dot(dpdx, eastT);
  float a12 = dot(dpdx, northT);
  float a21 = dot(dpdy, eastT);
  float a22 = dot(dpdy, northT);
  float det = a11 * a22 - a12 * a21;
  float slopeE = 0.0;
  float slopeN = 0.0;
  float slopeGrad = 0.0;
  if (abs(det) > 1e-10) {
    float dEdE = (a22 * dxE - a21 * dyE) / det;
    float dEdN = (-a12 * dxE + a11 * dyE) / det;
    float dRdE = (a22 * dxR - a21 * dyR) / det;
    float dRdN = (-a12 * dxR + a11 * dyR) / det;
    slopeGrad = clamp(length(vec2(dEdE, dEdN)) / 36000.0, 0.0, 1.0);
    slopeE = clamp((dEdE * 0.000028 + dRdE * 0.85) * quadValid, -0.62, 0.62);
    slopeN = clamp((dEdN * 0.000028 + dRdN * 0.85) * quadValid, -0.62, 0.62);
  }

  // Close-zoom world-space micro-bump from rock scree, hills, and woodland canopies.
  float microEps = 0.0006;
  float hillMicro0 = vnoise3(unitS * 210.0) * 0.55 + ridgedFbm3(unitS * 140.0) * 0.45;
  float hillMicroE = vnoise3(normalize(unitS + eastT * microEps) * 210.0) * 0.55
    + ridgedFbm3(normalize(unitS + eastT * microEps) * 140.0) * 0.45;
  float hillMicroN = vnoise3(normalize(unitS + northT * microEps) * 210.0) * 0.55
    + ridgedFbm3(normalize(unitS + northT * microEps) * 140.0) * 0.45;
  float microBumpE = clamp((hillMicroE - hillMicro0) * (0.28 + 0.52 * closeDetail01) / microEps * 0.00045, -0.35, 0.35);
  float microBumpN = clamp((hillMicroN - hillMicro0) * (0.28 + 0.52 * closeDetail01) / microEps * 0.00045, -0.35, 0.35);

  vec3 reliefWorldNorm = normalize(
    worldNorm - eastT * (slopeE * 0.72 + microBumpE) - northT * (slopeN * 0.72 + microBumpN)
  );
  float rawNdl = clamp(dot(worldNorm, sunDir), 0.0, 1.0);
  float reliefNdl = clamp(dot(reliefWorldNorm, sunDir), 0.0, 1.0);

  vec3 satRgb;
  if (isOceanPixel) {
    float depthM = max(0.0, -effectiveElevM);
    vec3 surfLittoral = vec3(0.075, 0.245, 0.275);
    vec3 surfShelf = vec3(0.028, 0.118, 0.198);
    vec3 surfAbyss = vec3(0.012, 0.042, 0.102);

    float shelfT = smoothstep(0.0, 185.0, depthM);
    float abyssT = smoothstep(160.0, 2200.0, depthM);
    vec3 waterRgb = mix(surfLittoral, surfShelf, shelfT);
    waterRgb = mix(waterRgb, surfAbyss, abyssT);

    // Coastal shallow river-mouth silt plume when continental discharge feeds shelf waters.
    float mouthMask = (1.0 - smoothstep(0.0, 95.0, depthM)) * smoothstep(1.2, 3.2, strahler + rainNorm * 0.8);
    vec3 siltCol = vec3(0.165, 0.265, 0.235);
    waterRgb = mix(waterRgb, siltCol, mouthMask * 0.52);

    // Close-zoom shoreline surf-foam breakers in shallow littoral waters (0..14m).
    float surfBand = (1.0 - smoothstep(0.5, 14.0, depthM)) * smoothstep(0.04, 0.25, landFrac);
    float breakerLine = smoothstep(0.42, 0.78, sin(depthM * 1.15 + vnoise3(unitS * 260.0) * 5.2));
    waterRgb = mix(waterRgb, vec3(0.76, 0.84, 0.88), surfBand * breakerLine * closeDetail01 * 0.65);

    // Polar & sub-freezing seasonal pack ice with fractured lead pattern.
    float iceLead = smoothstep(0.42, 0.62, fbm4(unitS * 38.0 + vec3(5.0, -2.0, 3.0)));
    float seaIceMask = smoothstep(-5.5, -13.5, tempC) * (0.45 + 0.55 * iceLead);
    waterRgb = mix(waterRgb, vec3(0.82, 0.87, 0.92), seaIceMask);

    // Wave micro-glint specular reflection in world tangent frame.
    float waveFreq = mix(120.0, 380.0, closeDetail01);
    float waveRipple = vnoise3(unitS * waveFreq) - 0.5;
    vec3 waterNorm = normalize(worldNorm + eastT * (waveRipple * 0.08) - northT * (waveRipple * 0.08));
    vec3 viewVec = normalize(mix(vec3(0.0, 0.0, 1.0), worldNorm, 0.45));
    vec3 halfVec = normalize(sunDir + viewVec);
    float spec = pow(clamp(dot(waterNorm, halfVec), 0.0, 1.0), 42.0) * (1.0 - seaIceMask) * 0.34;
    satRgb = waterRgb * (0.48 + 0.58 * rawNdl) + vec3(0.85, 0.88, 0.90) * spec;
  } else {
    float warmth = clamp((tempC + 8.0) / 34.0, 0.0, 1.0);
    float moist = clamp((aridity - 0.15) / 1.15, 0.0, 1.0);
    float microVar = fbm4(unitS * 52.0);
    float canopyDetail = fbm4(unitS * 115.0 + vec3(-3.1, 6.4, 2.2));
    float crownNoise = vnoise3(unitS * 360.0) * 0.6 + vnoise3(unitS * 720.0) * 0.4;
    float crownAo = mix(1.0, 0.72 + 0.52 * crownNoise, closeDetail01);

    // ESA Sentinel-2 Europe biophysical palette with close-zoom forest crown variation:
    vec3 ergSand = mix(vec3(0.66, 0.54, 0.36), vec3(0.74, 0.63, 0.44), microVar);
    vec3 medScrub = mix(vec3(0.24, 0.29, 0.16), vec3(0.42, 0.38, 0.22), microVar);
    vec3 tempPasture = mix(vec3(0.18, 0.31, 0.13), vec3(0.27, 0.38, 0.17), canopyDetail);
    vec3 tempForest = mix(vec3(0.085, 0.185, 0.080), vec3(0.135, 0.245, 0.110), canopyDetail) * crownAo;
    vec3 borealTaiga = mix(vec3(0.062, 0.130, 0.075), vec3(0.095, 0.175, 0.105), canopyDetail) * crownAo;
    vec3 tundraHeath = mix(vec3(0.22, 0.24, 0.18), vec3(0.32, 0.31, 0.25), microVar);

    // Zoom-revealed cultivated field patchwork with plowed furrows & dark hedgerows (UI-GLB-006).
    float zoomAgMask = smoothstep(0.42, 0.04, uCameraAltitudeNorm)
      * smoothstep(0.45, 0.72, fertility)
      * smoothstep(0.35, 0.10, slopeGrad)
      * smoothstep(4.0, 10.0, tempC);
    vec3 parcelInfo = fieldParcelPattern(unitS);
    float parcelFactor = parcelInfo.x;
    float hedgeMask = parcelInfo.y;
    float furrowMask = parcelInfo.z;
    vec3 croppedLowland = mix(tempForest, tempPasture, 0.45 + 0.45 * fertility);
    vec3 parcelAlbedo = mix(vec3(0.34, 0.32, 0.19), vec3(0.22, 0.37, 0.15), parcelFactor);
    parcelAlbedo *= mix(1.0, 0.90 + 0.18 * furrowMask, closeDetail01);
    parcelAlbedo = mix(parcelAlbedo, vec3(0.08, 0.17, 0.07), hedgeMask * closeDetail01 * 0.72);
    vec3 temperateAlbedo = mix(croppedLowland, parcelAlbedo, zoomAgMask * 0.72);

    // Smooth 2D Whittaker interpolation across thermal & moisture gradients.
    vec3 coldBelt = mix(tundraHeath, borealTaiga, smoothstep(0.25, 0.58, moist));
    vec3 tempBelt = mix(medScrub, temperateAlbedo, smoothstep(0.30, 0.65, moist));
    vec3 warmBelt = mix(ergSand, medScrub, smoothstep(0.18, 0.48, moist));
    warmBelt = mix(warmBelt, tempForest, smoothstep(0.55, 0.92, moist));

    vec3 landAlbedo = mix(coldBelt, tempBelt, smoothstep(0.14, 0.44, warmth));
    landAlbedo = mix(landAlbedo, warmBelt, smoothstep(0.54, 0.84, warmth));

    // Exposed mountain bedrock, coastal cliff bluffs & fractured scree bands.
    float strataBand = 0.5 + 0.5 * sin(effectiveElevM * 0.032 + microVar * 4.5 + strataRough * 0.7);
    float screeDetail = ridgedFbm3(unitS * 175.0) * closeDetail01;
    vec3 rockCol = mix(vec3(0.30, 0.28, 0.27), vec3(0.46, 0.44, 0.41), clamp(strataBand * 0.7 + screeDetail * 0.35, 0.0, 1.0));
    float rockMask = smoothstep(0.20, 0.66, slopeGrad + smoothstep(780.0, 2300.0, effectiveElevM) * 0.55);
    landAlbedo = mix(landAlbedo, rockCol, clamp(rockMask, 0.0, 0.92));

    // Alpine snowline, aspect shading & live weather snow accumulation.
    float snowTempScore = smoothstep(4.5, -5.0, tempC - effectiveElevM * 0.0022);
    float snowLiveScore = smoothstep(0.04, 0.28, snowPackM);
    float snowMask = clamp(max(snowTempScore * (1.0 - slopeGrad * 0.55), snowLiveScore) * (0.75 + 0.35 * microVar), 0.0, 1.0);
    vec3 snowCol = mix(vec3(0.82, 0.87, 0.93), vec3(0.95, 0.96, 0.98), reliefNdl);
    landAlbedo = mix(landAlbedo, snowCol, snowMask);

    // Coastal sandy shingle / narrow beach transition right at 0..18m.
    float beachMask = (1.0 - smoothstep(0.0, 16.0, effectiveElevM)) * smoothstep(0.10, 0.40, microVar);
    vec3 beachCol = mix(vec3(0.54, 0.49, 0.38), vec3(0.34, 0.32, 0.27), clamp(strataRough / 12.0, 0.0, 1.0));
    landAlbedo = mix(landAlbedo, beachCol, beachMask * (1.0 - snowMask) * 0.65);

    satRgb = landAlbedo * (0.34 + 0.74 * reliefNdl);
    // Subtle Rayleigh haze over high mountains and far atmospheric limb (3D sphere mode).
    satRgb += vec3(0.15, 0.26, 0.42) * fresnelRim * sunLimb * (1.0 - uProjectionMorph) * 0.34;
  }
  satRgb = mix(satRgb, isOceanPixel ? uOceanDeepRgb : uLandLowRgb, 0.06);

  // Optional selectable Antique Parchment Cartographic Lens (uParchmentMode >= 0.5).
  vec3 mapPaper = uUnexploredVellumRgb * (0.94 + 0.11 * grainLum);
  vec3 chartLandTint = mix(uLandLowRgb, uLandMidRgb, smoothstep(0.0, 2600.0, max(0.0, effectiveElevM)));
  vec3 parchmentRgb = isOceanPixel
    ? mix(mapPaper, uOceanShallowRgb, 0.45)
    : mix(mapPaper, chartLandTint, 0.34);

  float elevSteps = max(0.0, effectiveElevM / 360.0);
  float isolineDist = abs(fract(elevSteps) - 0.5);
  float fw = max(0.02, fwidth(elevSteps));
  float contourLine = isOceanPixel ? 0.0 : (1.0 - smoothstep(0.03, 0.03 + fw * 1.4, isolineDist));
  float coastLine = 1.0 - smoothstep(8.0, 34.0, abs(effectiveElevM));
  float hatchMask = isOceanPixel ? 0.0 : smoothstep(700.0, 2600.0, effectiveElevM) * hatchInk * 0.65;
  parchmentRgb = mix(parchmentRgb, uContourInkRgb, clamp(contourLine * 0.55 + coastLine * 0.85 + hatchMask, 0.0, 0.9));
  parchmentRgb = mix(parchmentRgb, uCompassRoseRgb, 0.16 * grat);

  vec3 finalRgb = mix(satRgb, parchmentRgb, uParchmentMode);

  // Weather overlay wetness & storm rainfall shading (UI-MAP-004).
  if (rainNorm > 0.05) {
    finalRgb *= (1.0 - 0.14 * rainNorm);
  }
  if (uWeatherOverlay > 0.5 && rainNorm > 0.05) {
    finalRgb = mix(finalRgb, uRiverWaterRgb, clamp(rainNorm * 0.55, 0.0, 0.55));
  }

  // 4-tier epistemic washes: Rumored sketch, Stale dust, Dated Survey gentle mute.
  if (isRumored > 0.5) {
    float luma = dot(finalRgb, vec3(0.299, 0.587, 0.114));
    vec3 sketchBase = mix(twilightBase, uUnexploredVellumRgb, uParchmentMode);
    vec3 sketch = mix(sketchBase, vec3(luma), 0.34);
    finalRgb = mix(sketch, uContourInkRgb, 0.08 * grat + 0.08 * hatchInk);
  } else if (isStale > 0.5) {
    float luma = dot(finalRgb, vec3(0.299, 0.587, 0.114));
    vec3 dustTarget = mix(vec3(luma) * 0.72 + vec3(0.07, 0.08, 0.10), mix(vec3(luma), uUnexploredVellumRgb, 0.55), uParchmentMode);
    finalRgb = mix(finalRgb, dustTarget, 0.58);
  } else if (isDatedSurvey > 0.5) {
    float luma = dot(finalRgb, vec3(0.299, 0.587, 0.114));
    finalRgb = mix(finalRgb, vec3(luma), 0.16);
  }

  // Blend across cloud-bank fog horizon strictly into the surveyed interior.
  float wispPattern = smoothstep(0.35, 0.78, fbm4(unitS * 26.0 + vec3(-4.0, 7.2, 1.8)));
  vec3 cloudWispColor = mix(vec3(0.68, 0.73, 0.80) * sunLimb, uUnexploredVellumRgb, uParchmentMode);
  finalRgb = mix(unmappedColor, finalRgb, fogReveal);
  finalRgb = mix(finalRgb, cloudWispColor, cloudMask * wispPattern * (0.48 - 0.25 * uParchmentMode));

  // Hover and selection circular reticles.
  if (hoverRing > 0.05) {
    vec3 hCol = mix(vec3(0.95, 0.78, 0.42), uSettlementMarkerRgb, uParchmentMode);
    finalRgb = mix(finalRgb, hCol, hoverRing * (0.60 + 0.20 * hatchInk));
  }
  if (selectRing > 0.05) {
    vec3 sCol = mix(vec3(0.98, 0.88, 0.54), uCompassRoseRgb, uParchmentMode);
    finalRgb = mix(finalRgb, sCol, selectRing * 0.78);
  }

  fragColor = vec4(finalRgb, 1.0);
}
`;

/**
 * Builds the GPU shader pipeline for the spherical Goldberg globe and 2D flat projections with
 * double-buffered cell attribute & biophysical `DataTexture`s (`UI-GLB-002`, `UI-GLB-006`, `UI-GLB-007`).
 */
export function createGlobeShaderController(initialPalette?: GlobePalette): GlobeShaderController {
  const palette = initialPalette ?? defaultNeutralPalette();
  const texA = createCellDataTexture();
  const texB = createCellDataTexture();
  const bioA = createCellDataTexture();
  const bioB = createCellDataTexture();
  let activeIdx = 0;

  const grainFallback = createNeutralPatternTexture(232, 224, 206);
  const hatchFallback = createNeutralPatternTexture(215, 208, 194);
  const biomeFallback = createNeutralPatternTexture(220, 220, 210);

  const loadedTextures: THREE.Texture[] = [];
  const loader = typeof window !== "undefined" ? new THREE.TextureLoader() : null;

  const uniforms = {
    uCellDataTex: { value: texA.texture },
    uBiophysicalTex: { value: bioA.texture },
    uTexSize: { value: CELL_DATA_TEX_SIZE },
    uCartographicBlend: { value: 0.5 },
    uCameraAltitudeNorm: { value: 0.62 },
    uParchmentMode: { value: 0.0 },
    uProjectionMode: { value: 0.0 },
    uProjectionMorph: { value: 0.0 },
    uFocusUnit: { value: new THREE.Vector3(0, 0, 1) },
    uSunDir: { value: new THREE.Vector3(0.45, 0.68, 0.58).normalize() },
    uHoveredCellId: { value: -1 },
    uSelectedCellId: { value: -1 },
    uHoveredUnitXyz: { value: new THREE.Vector3(0, 0, 0) },
    uSelectedUnitXyz: { value: new THREE.Vector3(0, 0, 0) },
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
        // Retain fallback neutral texture on load error.
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
      const targetBio = nextIdx === 0 ? bioA : bioB;
      const buf = target.buffer;
      const bioBuf = targetBio.buffer;
      buf.fill(0);
      bioBuf.fill(0);

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
        const isOcean = Boolean(cell.isOcean || cell.elevationM < 0);
        buf[offset] = encodeElevationByte(cell.elevationM, isOcean);

        const weather = weatherByCell.get(cell.cellId);
        const precip = weather ? Math.max(0, Math.min(1, weather.precipMmHr / 25)) : 0;
        const wetness = weather
          ? Math.max(0, Math.min(1, Math.max(precip, (weather.mudIndex01 ?? 0) * 0.85)))
          : 0;
        buf[offset + 1] = cell.tier === "surveyed" ? Math.round(wetness * 255) : 0;
        buf[offset + 2] = encodeBiomeByte(cell);

        let alpha = 42;
        if (cell.tier === "rumored") {
          alpha = 42;
        } else if (cell.tier === "stale") {
          alpha = 85;
        } else {
          alpha = cell.isLiveHqSight ? 192 : 170;
        }
        if (cell.cellId === selectedCellId || cell.cellId === hoveredCellId) {
          alpha = 255;
        } else if (cell.tier === "surveyed" && precip > 0.15) {
          alpha = 212;
        }
        buf[offset + 3] = alpha;

        const [bioR, bioG, bioB, bioAVal] = encodeBiophysicalBytes(cell, weather);
        bioBuf[offset] = bioR;
        bioBuf[offset + 1] = bioG;
        bioBuf[offset + 2] = bioB;
        bioBuf[offset + 3] = bioAVal;
      }

      target.texture.needsUpdate = true;
      targetBio.texture.needsUpdate = true;
      activeIdx = nextIdx;
      uniforms.uCellDataTex.value = target.texture;
      uniforms.uBiophysicalTex.value = targetBio.texture;
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
      hoveredUnitXyz?: readonly [number, number, number] | null,
      selectedUnitXyz?: readonly [number, number, number] | null,
    ) {
      uniforms.uCartographicBlend.value = Math.max(0, Math.min(1, blend01));
      uniforms.uSunDir.value.copy(sunDir).normalize();
      uniforms.uHoveredCellId.value = hoveredCellId ?? -1;
      uniforms.uSelectedCellId.value = selectedCellId ?? -1;
      if (hoveredUnitXyz) {
        uniforms.uHoveredUnitXyz.value.set(hoveredUnitXyz[0], hoveredUnitXyz[1], hoveredUnitXyz[2]);
      } else {
        uniforms.uHoveredUnitXyz.value.set(0, 0, 0);
      }
      if (selectedUnitXyz) {
        uniforms.uSelectedUnitXyz.value.set(selectedUnitXyz[0], selectedUnitXyz[1], selectedUnitXyz[2]);
      } else {
        uniforms.uSelectedUnitXyz.value.set(0, 0, 0);
      }
      uniforms.uWeatherOverlay.value = activeWeatherOverlay ? 1 : 0;
    },
    setAltitudeNorm(altitudeNorm01: number) {
      uniforms.uCameraAltitudeNorm.value = Math.max(0, Math.min(1, altitudeNorm01));
    },
    setParchmentMode(enabled: boolean) {
      uniforms.uParchmentMode.value = enabled ? 1 : 0;
    },
    setProjectionState(mode: GlobeProjectionMode, morph01: number, focusUnit: Vec3) {
      uniforms.uProjectionMode.value = mode === "globe" ? 0 : mode === "equal_area" ? 1 : 2;
      uniforms.uProjectionMorph.value = Math.max(0, Math.min(1, morph01));
      uniforms.uFocusUnit.value.set(focusUnit[0], focusUnit[1], focusUnit[2]);
    },
    dispose() {
      texA.texture.dispose();
      texB.texture.dispose();
      bioA.texture.dispose();
      bioB.texture.dispose();
      grainFallback.dispose();
      hatchFallback.dispose();
      biomeFallback.dispose();
      for (const lt of loadedTextures) lt.dispose();
      material.dispose();
    },
  };
}
