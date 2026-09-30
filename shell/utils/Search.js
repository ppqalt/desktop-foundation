.pragma library

function normalize(value) {
    return String(value || "").toLocaleLowerCase().trim();
}

function wordScore(haystack, needle) {
    if (haystack === needle) return 1000;
    if (haystack.startsWith(needle)) return 800 - haystack.length;
    const position = haystack.indexOf(needle);
    if (position >= 0) return 600 - position * 2 - haystack.length;
    let cursor = 0;
    let first = -1;
    let last = -1;
    for (let i = 0; i < haystack.length && cursor < needle.length; ++i) {
        if (haystack[i] === needle[cursor]) {
            if (first < 0) first = i;
            last = i;
            cursor++;
        }
    }
    return cursor === needle.length ? Math.max(1, 200 - first * 3 - (last - first) * 4) : -1;
}

function rank(entries, query) {
    const tokens = normalize(query).split(/\s+/).filter(Boolean);
    return entries.filter(entry => !entry.noDisplay && entry.command.length > 0).map(entry => {
        const name = normalize(entry.name);
        const extra = normalize([entry.genericName, entry.comment, ...(entry.keywords || [])].join(" "));
        let score = 0;
        for (const token of tokens) {
            const secondary = extra.includes(token) ? 120 : -1;
            const best = Math.max(wordScore(name, token), secondary);
            if (best < 0) return { entry: entry, score: -1 };
            score += best;
        }
        return { entry: entry, score: score };
    }).filter(row => row.score >= 0).sort((a, b) => b.score - a.score || a.entry.name.localeCompare(b.entry.name) || a.entry.id.localeCompare(b.entry.id)).map(row => row.entry);
}
