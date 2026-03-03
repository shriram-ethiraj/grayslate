import { createAutocompleteProvider, type AutocompleteConfig } from "../../extensions/autocompleteFactory";
// No lucide imports anymore!

export const markdownAutocompleteConfig: AutocompleteConfig = {
    triggerRegex: /\/\w*/,
    validForRegex: /^\/\w*$/,
    items: [
        { snippet: "# ${}", label: "/h1", iconData: "lucide:heading-1", detailText: "Heading 1" },
        { snippet: "## ${}", label: "/h2", iconData: "lucide:heading-2", detailText: "Heading 2" },
        { snippet: "### ${}", label: "/h3", iconData: "lucide:heading-3", detailText: "Heading 3" },
        { snippet: "#### ${}", label: "/h4", iconData: "lucide:heading-4", detailText: "Heading 4" },
        { snippet: "##### ${}", label: "/h5", iconData: "lucide:heading-5", detailText: "Heading 5" },
        { snippet: "###### ${}", label: "/h6", iconData: "lucide:heading-6", detailText: "Heading 6" },
        { snippet: "**${text}**", label: "/bold", iconData: "lucide:bold", detailText: "Bold text" },
        { snippet: "*${text}*", label: "/italic", iconData: "lucide:italic", detailText: "Italic text" },
        { snippet: "~~${text}~~", label: "/strike", iconData: "lucide:strikethrough", detailText: "Strikethrough" },
        { snippet: "> ${}", label: "/quote", iconData: "lucide:text-quote", detailText: "Blockquote" },
        { snippet: "```${language}\n${}\n```", label: "/code", iconData: "lucide:code", detailText: "Code block" },
        { snippet: "- ${}", label: "/ul", iconData: "lucide:list", detailText: "Bulleted list" },
        { snippet: "1. ${}", label: "/ol", iconData: "lucide:list-ordered", detailText: "Numbered list" },
        { snippet: "- [ ] ${}", label: "/task", iconData: "lucide:list-todo", detailText: "Task list" },
        { snippet: "[${text}](${url})", label: "/link", iconData: "lucide:link", detailText: "Link" },
        { snippet: "![${alt}](${url})", label: "/image", iconData: "lucide:image", detailText: "Image" },
        { snippet: "| ${Column 1} | ${Column 2} |\n| -------- | -------- |\n| ${Text}     | ${Text}     |", label: "/table", iconData: "lucide:table", detailText: "Table" },
        { snippet: "---\n", label: "/hr", iconData: "lucide:minus", detailText: "Horizontal rule" },
    ]
};

export const markdownAutocompleteProvider = createAutocompleteProvider(markdownAutocompleteConfig);
