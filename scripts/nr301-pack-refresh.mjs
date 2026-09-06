import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const packRoot = path.join(root, "resources/skills/novelist/v1");
const writeMode = process.argv.includes("--write");
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const stableJson = (value) => `${JSON.stringify(value, null, 2)}\n`;

const source = {
  id: "novelist",
  version: "0.2.0",
  repository: "https://github.com/comfuture/novelist",
  commit: "64e4a2e1473ff0bad5893eff394ccbf1fa1cd580",
  archiveSha256: "8b7c53e1832ee91fab29a320effe11efff9d3f07735b4ce8b3a541266bae85b8",
};

const capabilities = [
  {
    id: "create-novel-project",
    description: "规划安全、自包含的 Markdown 小说工程结构与初始约束。",
    usage: "当对话需要讨论新工程结构、初始元数据或目录约定时采用。",
    instructionResource: "instructions/create-novel-project.md",
    templateResources: ["fixtures/project.template.md"],
    referenceResources: [],
    readScopes: ["project", "style"],
  },
  {
    id: "create-setting",
    description: "分析和完善世界、地点、时代、社会规则、限制与氛围。",
    usage: "当对话涉及世界观、地点、时代或规则的一致性时采用。",
    instructionResource: "instructions/create-setting.md",
    templateResources: ["fixtures/setting.template.md"],
    referenceResources: [],
    readScopes: ["project", "characters", "materials", "plot", "style", "world", "chapters"],
  },
  {
    id: "create-character",
    description: "分析和完善人物、关系、动机、秘密、冲突与剧情功能。",
    usage: "当对话涉及人物设计、人物变化或人物连续性时采用。",
    instructionResource: "instructions/create-character.md",
    templateResources: ["fixtures/character.template.md"],
    referenceResources: [],
    readScopes: ["project", "characters", "materials", "plot", "style", "world", "chapters"],
  },
  {
    id: "create-material",
    description: "分析和整理主题、物件、线索、研究笔记及场景素材。",
    usage: "当对话产生或检查可复用小说素材时采用。",
    instructionResource: "instructions/create-material.md",
    templateResources: ["fixtures/material.template.md"],
    referenceResources: [],
    readScopes: ["project", "characters", "materials", "macguffins", "plot", "world", "chapters"],
  },
  {
    id: "create-plot",
    description: "分析和完善主剧情、支线、转折、揭示、回收与未决问题。",
    usage: "当对话涉及情节因果、结构、转折或回收时采用。",
    instructionResource: "instructions/create-plot.md",
    templateResources: ["fixtures/plot.template.md"],
    referenceResources: [],
    readScopes: [
      "project",
      "characters",
      "materials",
      "macguffins",
      "plot",
      "outlines",
      "world",
      "chapters",
    ],
  },
  {
    id: "novel-story-telling",
    description: "管理读者认知、章间流动、连续性、伏笔、草稿与修订方法。",
    usage: "当对话需要规划、续写、修订章节或检查叙事连续性时采用。",
    instructionResource: "instructions/novel-story-telling.md",
    templateResources: ["fixtures/chapter.template.md", "fixtures/outline.template.md"],
    referenceResources: [
      "references/storytelling/drama.md",
      "references/storytelling/mystery.md",
      "references/storytelling/romantic-entanglement.md",
      "references/storytelling/science-fiction.md",
      "references/storytelling/story-flow.md",
      "references/storytelling/time-travel-and-loop.md",
      "references/storytelling/wuxia.md",
    ],
    readScopes: [
      "project",
      "characters",
      "materials",
      "macguffins",
      "plot",
      "outlines",
      "world",
      "style",
      "chapters",
    ],
  },
  {
    id: "analytical-review",
    description: "对大纲、章节、全稿或修订结论进行证据化独立评审。",
    usage: "仅在作者明确要求评审、校对或回归验证时采用。",
    instructionResource: "instructions/analytical-review.md",
    templateResources: [],
    referenceResources: ["references/analytical-review-method.zh-CN.md"],
    readScopes: [
      "project",
      "characters",
      "materials",
      "macguffins",
      "plot",
      "outlines",
      "world",
      "style",
      "chapters",
    ],
  },
  {
    id: "create-visual-asset",
    description: "规划封面、插图、视觉风格、构图与提示词。",
    usage: "当对话需要视觉规格或提示词规划时采用；本阶段不生成或导入资产。",
    instructionResource: "instructions/create-visual-asset.md",
    templateResources: [],
    referenceResources: [],
    readScopes: ["project", "characters", "chapters", "style", "world"],
  },
  {
    id: "publish-novel",
    description: "依据明确出版要求，读取方法后调用当前原生 Host 完成本地 EPUB 预检或构建。",
    usage: "仅在作者明确讨论出版、导出、构建或 EPUB 验证时采用。",
    instructionResource: "instructions/publish-novel.md",
    templateResources: [],
    referenceResources: [],
    readScopes: ["project", "chapters", "style"],
  },
];

const pack = {
  schemaVersion: 1,
  sourcePackage: source,
  projection: {
    id: "novelgainker.novelist",
    schemaVersion: 1,
    contentVersion: "1.4.0",
  },
  provenanceResource: "provenance.lock.json",
  licenseResource: "LICENSE.upstream",
  attributionResource: "ATTRIBUTION.md",
  inventoryResource: "inventory.json",
  capabilityManifests: capabilities.map(({ id }) => `manifests/${id}.json`),
};

const outputs = new Map([["pack.json", stableJson(pack)]]);
for (const capability of capabilities) {
  outputs.set(`manifests/${capability.id}.json`, stableJson({ schemaVersion: 1, ...capability }));
}

const collectFiles = (directory, prefix = "") =>
  readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      return entry.isDirectory()
        ? collectFiles(path.join(directory, entry.name), relative)
        : [relative];
    })
    .filter((relative) => relative !== "inventory.json")
    .sort();

const effectiveContent = (relative) =>
  outputs.get(relative) ?? readFileSync(path.join(packRoot, relative), "utf8");

for (const [relative, expected] of outputs) {
  const absolute = path.join(packRoot, relative);
  if (writeMode) {
    writeFileSync(absolute, expected, "utf8");
  } else if (readFileSync(absolute, "utf8") !== expected) {
    throw new Error(`NR-301 generated resource drift: ${relative}`);
  }
}

const files = collectFiles(packRoot).map((relative) => {
  const content = effectiveContent(relative);
  return {
    path: relative,
    sha256: sha256(content),
    bytes: Buffer.byteLength(content),
  };
});
const aggregate = files
  .map(({ path: relative, bytes, sha256: hash }) => `${relative}\t${bytes}\t${hash}\n`)
  .join("");
const inventory = stableJson({
  schemaVersion: 1,
  algorithm: "sha256",
  excludedResources: ["inventory.json"],
  fileCount: files.length,
  totalBytes: files.reduce((total, file) => total + file.bytes, 0),
  packSha256: sha256(aggregate),
  files,
});
const inventoryPath = path.join(packRoot, "inventory.json");
if (writeMode) {
  writeFileSync(inventoryPath, inventory, "utf8");
} else if (readFileSync(inventoryPath, "utf8") !== inventory) {
  throw new Error("NR-301 Inventory drift; run pnpm pack:nr301:write");
}

for (const relative of collectFiles(packRoot)) {
  const absolute = path.join(packRoot, relative);
  if (!statSync(absolute).isFile()) throw new Error(`NR-301 non-file resource: ${relative}`);
}

console.log(
  `[nr-301] ${writeMode ? "wrote" : "verified"} ${capabilities.length} capabilities / ${files.length} resources / ${sha256(aggregate)}`,
);
