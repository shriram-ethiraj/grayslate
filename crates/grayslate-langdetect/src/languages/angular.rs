use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition()-> LanguageDefinition {
    LanguageDefinition {
        name: "angular",
        extensions: &[],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        keywords: &[
            "@component", "@injectable", "@ngmodule", "@directive", "@pipe",
            "@input", "@output", "@viewchild", "@hostlistener", "@hostbinding",
            "ngoninit", "ngondestroy", "ngafterviewinit", "ngonchanges",
        ],
        builtins: &[
            "httpclient", "formbuilder", "formgroup", "formcontrol",
            "activatedroute", "router", "observable", "subject",
            "behaviorsubject", "eventemitter", "changedetectorref",
            "elementref", "templateref", "viewcontainerref",
        ],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"@Component\s*\(\{", 5),
            wp!(r"@Injectable\s*\(", 5),
            wp!(r"@NgModule\s*\(", 5),
            wp!(r"\*ngIf=", 5),
            wp!(r"\*ngFor=", 5),
            wp!(r"\[\(ngModel\)\]", 5),
            wp!(r"@Directive\s*\(", 4),
            wp!(r"@Pipe\s*\(", 4),
        ],
        hints: &[
            wp!(r"@Input\s*\(", 3),
            wp!(r"@Output\s*\(", 3),
            wp!(r"@ViewChild\s*\(", 3),
            wp!(r"\(click\)=", 3),
            wp!(r"\[class\.\w+\]=", 3),
            wp!(r"\bnew\s+Form(?:Group|Control|Array)\b", 3),
            wp!(r"\btemplateUrl\s*:", 2),
            wp!(r"\bstyleUrls\s*:", 2),
            wp!(r"\bngOnInit\b", 2),
        ],
        disqualifiers: &[],
    }
}
