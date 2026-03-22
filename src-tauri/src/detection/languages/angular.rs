use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "angular",
        extensions: &[],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"@Component\s*\(\{", 5),
            wp!(r"@Injectable\s*\(", 5),
            wp!(r"@NgModule\s*\(", 5),
            wp!(r"@Directive\s*\(", 4),
            wp!(r"@Pipe\s*\(", 4),
            wp!(r"@Input\s*\(", 4),
            wp!(r"@Output\s*\(", 4),
            wp!(r"\*ngIf=", 5),
            wp!(r"\*ngFor=", 5),
            wp!(r"\[\(ngModel\)\]", 5),
            wp!(r"\(click\)=", 3),
            wp!(r"\[class\.\w+\]=", 3),
            wp!(r"\bnew\s+FormGroup\b", 3),
            wp!(r"\bnew\s+FormControl\b", 3),
        ],
        anti_patterns: &[
            wp!(r"\bv-if\b", -4),
            wp!(r"\{#if\b", -4),
        ],
        uses_hash_comments: false,
        keywords: &[
            "@Component", "@Injectable", "@NgModule", "@Directive", "@Pipe",
            "@Input", "@Output", "@ViewChild", "@HostListener", "@HostBinding",
            "ngOnInit", "ngOnDestroy", "ngAfterViewInit", "ngOnChanges",
        ],
        builtins: &[
            "HttpClient", "FormBuilder", "FormGroup", "FormControl",
            "ActivatedRoute", "Router", "Observable", "Subject",
            "BehaviorSubject", "EventEmitter", "ChangeDetectorRef",
            "ElementRef", "TemplateRef", "ViewContainerRef",
        ],
        family: None,
        exclusive_patterns: &[],
    }
}
