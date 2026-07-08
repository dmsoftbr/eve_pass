// Import parsers. The file is plaintext supplied by the user; we parse it in JS
// and hand structured items to Rust (which encrypts). Passwords live in memory
// only during this parse — the UI warns the user to delete the source file.

export interface EvepassItem {
  type: string;
  title: string;
  username: string;
  password: string;
  url: string;
  totp: string;
  notes: string;
  folders: string[];
  tags: string[];
  custom_fields: { name: string; value: string; hidden: boolean }[];
}

export interface ParsedImport {
  folderNames: string[];
  /** Item + the (optional) folder name it belongs to. */
  entries: { item: EvepassItem; folder?: string }[];
}

function blankItem(): EvepassItem {
  return {
    type: "login",
    title: "",
    username: "",
    password: "",
    url: "",
    totp: "",
    notes: "",
    folders: [],
    tags: [],
    custom_fields: [],
  };
}

// ── minimal RFC-4180-ish CSV parser (handles quotes, commas, newlines) ───────

export function parseCsv(text: string): string[][] {
  const rows: string[][] = [];
  let field = "";
  let row: string[] = [];
  let inQuotes = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (inQuotes) {
      if (c === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i++;
        } else inQuotes = false;
      } else field += c;
    } else if (c === '"') inQuotes = true;
    else if (c === ",") {
      row.push(field);
      field = "";
    } else if (c === "\n" || c === "\r") {
      if (c === "\r" && text[i + 1] === "\n") i++;
      row.push(field);
      field = "";
      if (row.some((f) => f !== "")) rows.push(row);
      row = [];
    } else field += c;
  }
  if (field !== "" || row.length) {
    row.push(field);
    if (row.some((f) => f !== "")) rows.push(row);
  }
  return rows;
}

// ── Bitwarden JSON ───────────────────────────────────────────────────────────

export function parseBitwarden(text: string): ParsedImport {
  const data = JSON.parse(text);
  const folderById = new Map<string, string>();
  const folderNames: string[] = [];
  for (const f of data.folders ?? []) {
    if (f.id && f.name) {
      folderById.set(f.id, f.name);
      if (!folderNames.includes(f.name)) folderNames.push(f.name);
    }
  }
  const entries: ParsedImport["entries"] = [];
  for (const it of data.items ?? []) {
    if (it.type !== 1 && it.login == null) continue; // logins only for the MVP
    const item = blankItem();
    item.title = it.name ?? "";
    item.username = it.login?.username ?? "";
    item.password = it.login?.password ?? "";
    item.url = it.login?.uris?.[0]?.uri ?? "";
    item.totp = it.login?.totp ?? "";
    item.notes = it.notes ?? "";
    item.custom_fields = (it.fields ?? []).map((f: any) => ({
      name: f.name ?? "",
      value: f.value ?? "",
      hidden: f.type === 1,
    }));
    const folder = it.folderId ? folderById.get(it.folderId) : undefined;
    entries.push({ item, folder });
  }
  return { folderNames, entries };
}

// ── generic / NordPass CSV via a column mapping ──────────────────────────────

export type CsvMapping = Record<"title" | "username" | "password" | "url" | "notes" | "folder", number>;

/** Guess a column index for each field from the header row. */
export function guessMapping(headers: string[]): CsvMapping {
  const find = (...names: string[]) => {
    const i = headers.findIndex((h) => names.includes(h.trim().toLowerCase()));
    return i;
  };
  return {
    title: find("name", "title", "item"),
    username: find("username", "user", "login", "email"),
    password: find("password", "pass"),
    url: find("url", "uri", "website", "web site"),
    notes: find("note", "notes"),
    folder: find("folder", "group", "category"),
  };
}

export function parseCsvWithMapping(rows: string[][], map: CsvMapping): ParsedImport {
  const body = rows.slice(1); // drop header
  const folderNames: string[] = [];
  const entries: ParsedImport["entries"] = [];
  const get = (row: string[], idx: number) => (idx >= 0 ? (row[idx] ?? "") : "");
  for (const row of body) {
    const item = blankItem();
    item.title = get(row, map.title) || "(sem título)";
    item.username = get(row, map.username);
    item.password = get(row, map.password);
    item.url = get(row, map.url);
    item.notes = get(row, map.notes);
    const folder = get(row, map.folder) || undefined;
    if (folder && !folderNames.includes(folder)) folderNames.push(folder);
    entries.push({ item, folder });
  }
  return { folderNames, entries };
}
