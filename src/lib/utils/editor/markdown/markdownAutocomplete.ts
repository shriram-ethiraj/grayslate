import { createAutocompleteProvider, type AutocompleteConfig } from "../autocompleteFactory";
import {
    Heading1,
    Heading2,
    Heading3,
    Heading4,
    Heading5,
    Heading6,
    Bold,
    Italic,
    Strikethrough,
    TextQuote,
    Code,
    List,
    ListOrdered,
    ListTodo,
    Link,
    Image,
    Table,
    Minus
} from "lucide";

export const markdownAutocompleteConfig: AutocompleteConfig = {
    triggerRegex: /\/\w*/,
    validForRegex: /^\/\w*$/,
    items: [
        { snippet: "# ${}", label: "/h1", iconNode: Heading1, detailText: "Heading 1" },
        { snippet: "## ${}", label: "/h2", iconNode: Heading2, detailText: "Heading 2" },
        { snippet: "### ${}", label: "/h3", iconNode: Heading3, detailText: "Heading 3" },
        { snippet: "#### ${}", label: "/h4", iconNode: Heading4, detailText: "Heading 4" },
        { snippet: "##### ${}", label: "/h5", iconNode: Heading5, detailText: "Heading 5" },
        { snippet: "###### ${}", label: "/h6", iconNode: Heading6, detailText: "Heading 6" },
        { snippet: "**${text}**", label: "/bold", iconNode: Bold, detailText: "Bold text" },
        { snippet: "*${text}*", label: "/italic", iconNode: Italic, detailText: "Italic text" },
        { snippet: "~~${text}~~", label: "/strike", iconNode: Strikethrough, detailText: "Strikethrough" },
        { snippet: "> ${}", label: "/quote", iconNode: TextQuote, detailText: "Blockquote" },
        { snippet: "```${language}\n${}\n```", label: "/code", iconNode: Code, detailText: "Code block" },
        { snippet: "- ${}", label: "/ul", iconNode: List, detailText: "Bulleted list" },
        { snippet: "1. ${}", label: "/ol", iconNode: ListOrdered, detailText: "Numbered list" },
        { snippet: "- [ ] ${}", label: "/task", iconNode: ListTodo, detailText: "Task list" },
        { snippet: "[${text}](${url})", label: "/link", iconNode: Link, detailText: "Link" },
        { snippet: "![${alt}](${url})", label: "/image", iconNode: Image, detailText: "Image" },
        { snippet: "| ${Column 1} | ${Column 2} |\n| -------- | -------- |\n| ${Text}     | ${Text}     |", label: "/table", iconNode: Table, detailText: "Table" },
        { snippet: "---\n", label: "/hr", iconNode: Minus, detailText: "Horizontal rule" },
    ]
};

export const markdownAutocompleteProvider = createAutocompleteProvider(markdownAutocompleteConfig);
