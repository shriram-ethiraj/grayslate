import { ModelOperations, type ModelResult } from "@vscode/vscode-languagedetection";

class LanguageDetector {
    private modelOperations: ModelOperations | null = null;
    private initPromise: Promise<ModelOperations> | null = null;

    // We only map languages we currently support in our Editor.
    // If the model recommends something else, we ignore it for now or default to text.
    private supportedLanguages = new Set([
        'json', 'javascript', 'typescript', 'python', 'csv', 'markdown', 'text',
        'html', 'css', 'yaml', 'c', 'cpp', 'java', 'go', 'xml'
    ]);

    private getModelOperations(): Promise<ModelOperations> {
        if (this.modelOperations) {
            return Promise.resolve(this.modelOperations);
        }

        if (!this.initPromise) {
            this.initPromise = (async () => {
                const ops = new ModelOperations({
                    modelJsonLoaderFunc: async () => {
                        const response = await fetch('/model/model.json');
                        if (!response.ok) throw new Error("HTTP " + response.status);
                        return await response.json();
                    },
                    weightsLoaderFunc: async () => {
                        const response = await fetch('/model/group1-shard1of1.bin');
                        if (!response.ok) throw new Error("HTTP " + response.status);
                        return await response.arrayBuffer();
                    },
                    minContentSize: 5
                });

                // NOTE: runModel might need an empty warmup or the loaders need to be explicitly called
                // but the library does this internally on first runModel.
                this.modelOperations = ops;
                return ops;
            })();
        }

        return this.initPromise;
    }

    async detect(content: string): Promise<string | null> {
        if (!content || content.trim().length === 0) {
            return null;
        }

        // Extremely large files (100MB+) will crash heuristics / ML model and freeze the UI.
        // We only need the first ~50KB to accurately guess the language.
        const MAX_CONTENT_LENGTH = 50000;
        const boundedContent = content.length > MAX_CONTENT_LENGTH
            ? content.slice(0, MAX_CONTENT_LENGTH)
            : content;

        // 1. Production-grade Fast Heuristics for common data formats
        const heuristicMatch = this.detectByHeuristics(boundedContent);
        if (heuristicMatch) {
            return heuristicMatch;
        }

        if (boundedContent.trim().length < 5) {
            return null;
        }

        // 2. ML Model Fallback (for complex code like JS/Python/TS)
        try {
            const ops = await this.getModelOperations();
            const results: ModelResult[] = await ops.runModel(boundedContent);

            if (results && results.length > 0) {
                const bestMatch = this.mapLanguageId(results[0].languageId);

                // We require at least an 10% confidence score to switch automatically
                if (this.supportedLanguages.has(bestMatch) && results[0].confidence > 0.1) {
                    return bestMatch;
                }
            }
        } catch (error) {
            // Language ML detection failed silently
        }

        return null; // Not enough confidence or unsupported language
    }

    /**
     * Fast, basic heuristic checks for formats that the ML model struggles with, 
     * especially when the input is very short (e.g. `{"test": 1}`)
     */
    private detectByHeuristics(content: string): string | null {
        const trimmed = content.trim();
        if (!trimmed) return null;

        // JSON Heuristic: Fast try-catch parse
        if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
            try {
                JSON.parse(trimmed);
                return 'json';
            } catch (e) {
                // Not valid JSON, fall through
            }
        }

        // CSV Heuristic: Multiple lines with consistent commas, no obvious JSON/HTML wrappers
        // A very basic check: does the first line have commas, and do subsequent lines match the comma count?
        const lines = trimmed.split('\n').map(l => l.trim()).filter(l => l.length > 0);
        if (lines.length > 1 && lines.every(line => line.includes(','))) {
            const firstLineCommaCount = (lines[0].match(/,/g) || []).length;
            if (firstLineCommaCount > 0) {
                const isConsistentCsv = lines.every(line => (line.match(/,/g) || []).length === firstLineCommaCount);
                if (isConsistentCsv && !trimmed.startsWith('{') && !trimmed.startsWith('[')) {
                    return 'csv';
                }
            }
        }

        // Markdown Heuristic: Fast check for common markdown headers
        if (trimmed.startsWith('# ') || trimmed.startsWith('## ') || trimmed.startsWith('### ')) {
            return 'markdown';
        }

        // XML Heuristic: starts with an XML declaration or a tag
        if (trimmed.startsWith('<?xml') || trimmed.startsWith('<!--')) {
            return 'xml';
        }

        // HTML Heuristic: starts with <!DOCTYPE html or <html
        if (/^<!doctype\s+html/i.test(trimmed) || /^<html[\s>]/i.test(trimmed)) {
            return 'html';
        }

        // YAML Heuristic: document separator or key: value pattern
        if (trimmed.startsWith('---') || /^[a-zA-Z_][\w.-]*\s*:/m.test(trimmed)) {
            return 'yaml';
        }

        return null;
    }

    // Maps standard vs-languagedetection IDs to CodeMirror/Internal IDs
    private mapLanguageId(id: string): string {
        const mappings: Record<string, string> = {
            'js': 'javascript',
            'jsx': 'javascript',
            'ts': 'typescript',
            'tsx': 'typescript',
            'py': 'python',
            'md': 'markdown',
            'c': 'c',
            'cpp': 'cpp',
            'c++': 'cpp',
            'java': 'java',
            'go': 'go',
            'golang': 'go',
            'html': 'html',
            'css': 'css',
            'yaml': 'yaml',
            'yml': 'yaml',
            'xml': 'xml',
        };

        return mappings[id] ?? id;
    }
}

// Export a singleton instance
export const languageDetector = new LanguageDetector();
