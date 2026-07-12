import { createAutocompleteProvider, type AutocompleteConfig } from "../../extensions/autocompleteFactory";
import Heading1 from "~icons/lucide/heading-1?raw";
import Heading2 from "~icons/lucide/heading-2?raw";
import Heading3 from "~icons/lucide/heading-3?raw";
import Heading4 from "~icons/lucide/heading-4?raw";
import Heading5 from "~icons/lucide/heading-5?raw";
import Heading6 from "~icons/lucide/heading-6?raw";
import Bold from "~icons/lucide/bold?raw";
import Italic from "~icons/lucide/italic?raw";
import Strikethrough from "~icons/lucide/strikethrough?raw";
import TextQuote from "~icons/lucide/text-quote?raw";
import Code from "~icons/lucide/code?raw";
import List from "~icons/lucide/list?raw";
import ListOrdered from "~icons/lucide/list-ordered?raw";
import ListTodo from "~icons/lucide/list-todo?raw";
import Link from "~icons/lucide/link?raw";
import Image from "~icons/lucide/image?raw";
import Table from "~icons/lucide/table?raw";
import Minus from "~icons/lucide/minus?raw";

// unplugin-icons' ~icons/* ambient types assume the component compiler; the ?raw
// query overrides that at build time to a plain SVG string, so the import type
// (Component) doesn't match the actual runtime value (string). Cast accordingly.
const svg = (icon: unknown) => icon as string;

export const markdownAutocompleteConfig: AutocompleteConfig = {
    triggerRegex: /\/\w*/,
    validForRegex: /^\/\w*$/,
    items: [
        { snippet: "# ${}", label: "/h1", iconData: svg(Heading1), detailText: "Heading 1" },
        { snippet: "## ${}", label: "/h2", iconData: svg(Heading2), detailText: "Heading 2" },
        { snippet: "### ${}", label: "/h3", iconData: svg(Heading3), detailText: "Heading 3" },
        { snippet: "#### ${}", label: "/h4", iconData: svg(Heading4), detailText: "Heading 4" },
        { snippet: "##### ${}", label: "/h5", iconData: svg(Heading5), detailText: "Heading 5" },
        { snippet: "###### ${}", label: "/h6", iconData: svg(Heading6), detailText: "Heading 6" },
        { snippet: "**${text}**", label: "/bold", iconData: svg(Bold), detailText: "Bold text" },
        { snippet: "*${text}*", label: "/italic", iconData: svg(Italic), detailText: "Italic text" },
        { snippet: "~~${text}~~", label: "/strike", iconData: svg(Strikethrough), detailText: "Strikethrough" },
        { snippet: "> ${}", label: "/quote", iconData: svg(TextQuote), detailText: "Blockquote" },
        { snippet: "```${language}\n${}\n```", label: "/code", iconData: svg(Code), detailText: "Code block" },
        { snippet: "- ${}", label: "/ul", iconData: svg(List), detailText: "Bulleted list" },
        { snippet: "1. ${}", label: "/ol", iconData: svg(ListOrdered), detailText: "Numbered list" },
        { snippet: "- [ ] ${}", label: "/task", iconData: svg(ListTodo), detailText: "Task list" },
        { snippet: "[${text}](${url})", label: "/link", iconData: svg(Link), detailText: "Link" },
        { snippet: "![${alt}](${url})", label: "/image", iconData: svg(Image), detailText: "Image" },
        { snippet: "| ${Column 1} | ${Column 2} |\n| -------- | -------- |\n| ${Text}     | ${Text}     |", label: "/table", iconData: svg(Table), detailText: "Table" },
        { snippet: "---\n", label: "/hr", iconData: svg(Minus), detailText: "Horizontal rule" },
    ]
};

export const markdownAutocompleteProvider = createAutocompleteProvider(markdownAutocompleteConfig);
