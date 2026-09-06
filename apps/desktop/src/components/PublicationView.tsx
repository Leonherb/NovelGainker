import { useState } from "react";
import {
  requestPublicationDetail,
  type PublicationDetail,
  type PublicationIssue,
  type PublicationReceipt,
} from "../lib/tauri/publication";

const labels: Record<PublicationReceipt["status"], string> = {
  generated: "EPUB 已生成（本地产物，未上架）",
  preflight_passed: "EPUB 预检通过（尚未生成）",
  validation_failed: "本次 EPUB 验证失败",
  stale: "EPUB 输入已过期，不能作为当前版本",
};
const messages: Record<PublicationIssue, string> = {
  metadata_required:
    "请在 project.md 中补齐书名 title、作者 authorDisplayName 和 writingLanguage: zh-CN。",
  no_final_chapters: "没有可出版的定稿章节；请先明确确认合格章节定稿。",
  final_confirmation_required: "部分 final 章节没有当前有效确认；核对正文后，请明确要求重新定稿。",
  invalid_chapter: "请修正章节元数据、重复章号、编码或 Draft 正文后再出版。",
  unsupported_markdown:
    "正文有暂不支持的语法；请检查表格、链接、原始 HTML 或嵌套标记。本次没有省略内容生成书籍。",
  invalid_asset: "请核对 assets 内引用图片是否存在，且是尺寸和结构合格的 PNG/JPEG。",
  unsafe_path: "存在不安全路径；请核对章节或图片位置，不要使用链接目录或项目外资源。",
  limit_exceeded: "达到已声明的出版输入、输出或存储上限；旧产物没有被自动删除。",
  source_changed: "书稿、图片或定稿确认已变化。请核对当前稿，必要时重新定稿，再提出新的构建要求。",
  local_validation_failed: "产物结构或 Hash 验证失败；请保留原稿，不要把该文件当作有效的新书。",
  store_unavailable: "本次结果需要重新核对；请保留旧产物，稍后重查，不要根据模型自述判断成功。",
  source_required: "本次缺少有效用户来源或方法读取依据；可以在新消息中明确提出预检或构建要求。",
  cancelled: "本次出版已取消，原稿和已有产物保留。",
};

export function PublicationView({
  projectSessionId,
  receipt,
}: {
  projectSessionId: string;
  receipt: PublicationReceipt;
}) {
  const [expanded, setExpanded] = useState(false);
  const [detail, setDetail] = useState<PublicationDetail | null>(null);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const [visibleInputs, setVisibleInputs] = useState(20);
  const shown = detail?.receipt ?? receipt;
  const load = async () => {
    if (busy) return;
    setBusy(true);
    setFailed(false);
    try {
      setDetail(await requestPublicationDetail(projectSessionId, receipt.artifactId));
    } catch {
      setFailed(true);
    } finally {
      setBusy(false);
    }
  };
  return (
    <section className="canon-change publication-result" aria-label="本地 EPUB 出版结果">
      <p>{labels[shown.status]}</p>
      {shown.issue ? <p role="status">{messages[shown.issue]}</p> : null}
      {shown.status === "generated" || shown.status === "preflight_passed" ? (
        <p>
          {shown.chapterCount} 章{shown.withoutCover ? " · 无封面 EPUB" : " · 含封面"}
          {shown.skippedDrafts ? `；另有 ${shown.skippedDrafts} 章草稿未导出` : ""}
        </p>
      ) : null}
      <button
        type="button"
        aria-expanded={expanded}
        aria-controls={`publication-${receipt.artifactId}`}
        onClick={() => {
          setExpanded(!expanded);
          if (!expanded) void load();
        }}
      >
        {expanded ? "收起出版依据" : "查看出版依据"}
      </button>
      {expanded ? (
        <div id={`publication-${receipt.artifactId}`}>
          {busy ? <p role="status">正在核对产物与当前输入……</p> : null}
          {failed ? (
            <p role="status">
              暂时无法核对；以上是先前回执，不代表当前产物仍有效。聊天可以继续。
              <button type="button" onClick={() => void load()} disabled={busy}>
                重新核对出版依据
              </button>
            </p>
          ) : null}
          {detail ? (
            <>
              {shown.relativePath ? (
                <p>
                  产物位置：<code>{shown.relativePath}</code>
                </p>
              ) : (
                <p>本次没有可确认的新产物文件。</p>
              )}
              {shown.outputHash ? (
                <p>
                  产物 SHA-256：<code>{shown.outputHash}</code>
                </p>
              ) : null}
              <p>{detail.validationScope}</p>
              <ul>
                {detail.inputs.slice(0, visibleInputs).map((input) => (
                  <li key={input.relativePath}>
                    <code>{input.relativePath}</code>
                    {input.confirmationId ? " · 绑定定稿确认" : ""}
                    <br />
                    <code>{input.contentHash}</code>
                  </li>
                ))}
              </ul>
              {visibleInputs < detail.inputs.length ? (
                <button type="button" onClick={() => setVisibleInputs((value) => value + 20)}>
                  显示更多输入依据
                </button>
              ) : null}
            </>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
