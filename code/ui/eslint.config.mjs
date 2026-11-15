import tsParser from "@typescript-eslint/parser";
import tsPlugin from "@typescript-eslint/eslint-plugin";

const cartridges = [
  "globe",
  "overlays",
  "windows",
  "views",
  "dispatches",
  "hud",
  "skins",
  "audio",
];

const skinAssetSyntaxRules = {
  "no-restricted-syntax": [
    "error",
    {
      selector: "Literal[value=/^#[0-9a-fA-F]{3,8}$/]",
      message: "Hardcoded hex colors are forbidden outside external skin CSS/manifests (UI-SKN-001).",
    },
    {
      selector: "Literal[value=/\\.(ogg|wav|mp3|flac)$/i]",
      message: "Hardcoded audio file paths are forbidden outside audio manifests (UI-AUD-002).",
    },
    {
      selector: "Literal[value=/\\b(?:rgb|rgba|hsl|hsla)\\s*\\(/i]",
      message: "Hardcoded rgb()/hsl() color literals are forbidden outside external skin CSS/manifests (UI-SKN-001).",
    },
    {
      selector: "TemplateElement[value.raw=/(?:^|\\s)#[0-9a-fA-F]{3,8}\\b/]",
      message: "Hardcoded hex colors in template literals are forbidden outside external skin CSS/manifests (UI-SKN-001).",
    },
    {
      selector: "TemplateElement[value.raw=/\\.(ogg|wav|mp3|flac)\\b/i]",
      message: "Hardcoded audio paths in template literals are forbidden outside audio manifests (UI-AUD-002).",
    },
  ],
};

function cartridgeIsolationRule(own) {
  const forbidden = cartridges.filter((c) => c !== own);
  return {
    files: [`src/${own}/**/*.ts`],
    languageOptions: {
      parser: tsParser,
      parserOptions: { sourceType: "module" },
    },
    plugins: { "@typescript-eslint": tsPlugin },
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            ...forbidden.map((sib) => ({
              group: [`../${sib}/*`, `../${sib}`, `./*/../${sib}/*`, `**/src/${sib}/**`],
              message: `Cartridge '${own}' must never import sibling '${sib}'. Use ../spine/ contracts only.`,
            })),
            {
              group: ["electron", "node:*", "fs", "path", "child_process"],
              message: `Renderer cartridge '${own}' cannot import Node or Electron APIs.`,
            },
          ],
        },
      ],
      ...skinAssetSyntaxRules,
    },
  };
}

export default [
  {
    ignores: ["dist/**", "dist-electron/**", "release/**", "node_modules/**"],
  },
  {
    files: ["src/spine/**/*.ts"],
    languageOptions: {
      parser: tsParser,
      parserOptions: { sourceType: "module" },
    },
    plugins: { "@typescript-eslint": tsPlugin },
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            ...cartridges.map((c) => ({
              group: [`../${c}/*`, `../${c}`, `**/src/${c}/**`],
              message: "src/spine/ must remain pure and never depend on feature cartridges.",
            })),
            {
              group: ["three", "electron", "node:*", "fs", "path", "child_process"],
              message: "src/spine/ cannot depend on Three.js, Electron, or Node APIs.",
            },
          ],
        },
      ],
      ...skinAssetSyntaxRules,
    },
  },
  {
    files: ["src/*.ts"],
    languageOptions: {
      parser: tsParser,
      parserOptions: { sourceType: "module" },
    },
    plugins: { "@typescript-eslint": tsPlugin },
    rules: {
      ...skinAssetSyntaxRules,
    },
  },
  ...cartridges.map(cartridgeIsolationRule),
  {
    files: ["electron/**/*.ts"],
    languageOptions: {
      parser: tsParser,
      parserOptions: { sourceType: "module" },
    },
    plugins: { "@typescript-eslint": tsPlugin },
    rules: {},
  },
];
