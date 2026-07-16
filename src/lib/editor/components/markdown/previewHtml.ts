import { invoke } from "$lib/ipc";

export interface PreparedMarkdownHtml {
  html: string;
  objectUrls: string[];
}

const URL_SCHEME = /^[a-zA-Z][a-zA-Z\d+.-]*:/;

const IMAGE_MIME_TYPES: Readonly<Record<string, string>> = {
  avif: "image/avif",
  bmp: "image/bmp",
  gif: "image/gif",
  ico: "image/x-icon",
  jpeg: "image/jpeg",
  jpg: "image/jpeg",
  png: "image/png",
  svg: "image/svg+xml",
  webp: "image/webp",
};

function stripUrlSuffix(value: string): string {
  const suffixIndex = value.search(/[?#]/);
  return suffixIndex === -1 ? value : value.slice(0, suffixIndex);
}

function isRelativeResource(value: string): boolean {
  const trimmed = value.trim();
  return (
    trimmed.length > 0 &&
    !trimmed.startsWith("#") &&
    !trimmed.startsWith("/") &&
    !trimmed.startsWith("//") &&
    !URL_SCHEME.test(trimmed)
  );
}

function imageMimeType(resourcePath: string): string {
  const path = stripUrlSuffix(resourcePath);
  const dotIndex = path.lastIndexOf(".");
  if (dotIndex === -1) return "application/octet-stream";
  return IMAGE_MIME_TYPES[path.slice(dotIndex + 1).toLowerCase()] ?? "application/octet-stream";
}

function githubHeadingSlug(text: string): string {
  const slug = text
    .trim()
    .toLocaleLowerCase()
    .replace(/[^\p{L}\p{M}\p{N}\s_-]/gu, "")
    .replace(/\s+/g, "-");
  return slug || "section";
}

function addHeadingIds(document: Document): void {
  const counts = new Map<string, number>();
  for (const heading of document.querySelectorAll<HTMLHeadingElement>("h1, h2, h3, h4, h5, h6")) {
    const base = githubHeadingSlug(heading.textContent ?? "");
    const duplicateIndex = counts.get(base) ?? 0;
    counts.set(base, duplicateIndex + 1);
    heading.id = duplicateIndex === 0 ? base : `${base}-${duplicateIndex}`;
  }
}

function wrapTables(document: Document): void {
  for (const table of document.querySelectorAll<HTMLTableElement>("table")) {
    const wrapper = document.createElement("div");
    wrapper.className = "markdown-table-scroll";
    table.replaceWith(wrapper);
    wrapper.append(table);
  }
}

async function rewriteSrcset(
  srcset: string,
  resolveLocalImage: (resourcePath: string) => Promise<string | null>,
): Promise<string> {
  const rewritten: string[] = [];

  for (const rawCandidate of srcset.split(",")) {
    const candidate = rawCandidate.trim();
    if (!candidate) continue;

    const [url, ...descriptorParts] = candidate.split(/\s+/);
    if (!isRelativeResource(url)) {
      rewritten.push(candidate);
      continue;
    }

    const resolved = await resolveLocalImage(url);
    if (!resolved) continue;
    rewritten.push([resolved, ...descriptorParts].join(" "));
  }

  return rewritten.join(", ");
}

/**
 * Adds trusted preview-only structure and resolves local image references
 * before the HTML is inserted into the live document. Parsing in a detached
 * document prevents relative image URLs from making accidental app-origin
 * requests while they are being rewritten.
 */
export async function prepareMarkdownPreviewHtml(
  html: string,
  authorization: { documentId: string; documentGeneration: number } | undefined,
): Promise<PreparedMarkdownHtml> {
  const document = new DOMParser().parseFromString(html, "text/html");
  const objectUrls = new Set<string>();
  const localImageCache = new Map<string, Promise<string | null>>();

  addHeadingIds(document);
  wrapTables(document);

  async function resolveLocalImage(resourcePath: string): Promise<string | null> {
    if (!authorization || !isRelativeResource(resourcePath)) return null;

    const existing = localImageCache.get(resourcePath);
    if (existing) return existing;

    const pending = invoke<ArrayBuffer>("read_markdown_preview_asset", {
      documentId: authorization.documentId,
      documentGeneration: authorization.documentGeneration,
      resourcePath,
    })
      .then((buffer) => {
        const objectUrl = URL.createObjectURL(
          new Blob([buffer], { type: imageMimeType(resourcePath) }),
        );
        objectUrls.add(objectUrl);
        return objectUrl;
      })
      .catch(() => null);
    localImageCache.set(resourcePath, pending);
    return pending;
  }

  const rewriteTasks: Promise<void>[] = [];

  for (const image of document.querySelectorAll<HTMLImageElement>("img")) {
    const src = image.getAttribute("src");
    if (src && isRelativeResource(src)) {
      rewriteTasks.push(
        resolveLocalImage(src).then((resolved) => {
          if (resolved) image.src = resolved;
          else image.removeAttribute("src");
        }),
      );
    }

    const srcset = image.getAttribute("srcset");
    if (srcset) {
      rewriteTasks.push(
        rewriteSrcset(srcset, resolveLocalImage).then((rewritten) => {
          if (rewritten) image.srcset = rewritten;
          else image.removeAttribute("srcset");
        }),
      );
    }
  }

  for (const source of document.querySelectorAll<HTMLSourceElement>("source")) {
    const src = source.getAttribute("src");
    if (src && isRelativeResource(src)) {
      rewriteTasks.push(
        resolveLocalImage(src).then((resolved) => {
          if (resolved) source.src = resolved;
          else source.removeAttribute("src");
        }),
      );
    }

    const srcset = source.getAttribute("srcset");
    if (srcset) {
      rewriteTasks.push(
        rewriteSrcset(srcset, resolveLocalImage).then((rewritten) => {
          if (rewritten) source.srcset = rewritten;
          else source.removeAttribute("srcset");
        }),
      );
    }
  }

  await Promise.all(rewriteTasks);
  return { html: document.body.innerHTML, objectUrls: [...objectUrls] };
}
