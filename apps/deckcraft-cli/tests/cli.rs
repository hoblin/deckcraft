use std::process::{Command, Output};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_deckcraft-cli")).args(args).output().unwrap()
}

fn stdout(o: &Output) -> String {
    assert!(o.status.success(), "exit {:?}\nstderr: {}", o.status, String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    assert!(!o.status.success(), "exit {:?}\nstdout: {}", o.status, String::from_utf8_lossy(&o.stdout));
    String::from_utf8_lossy(&o.stderr).into_owned()
}

const SUBCOMMANDS: [&str; 9] = ["info", "render", "convert", "run", "commands", "describe", "app", "mcp", "links"];

fn words(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn an_unknown_subcommand_is_named_before_the_overview() {
    for args in [vec!["frobnicate"], vec!["help", "frobnicate"], vec!["--info", "--help"]] {
        let err = stderr(&cli(&args));
        assert!(err.contains("unknown subcommand `") && err.contains("\n\ndeckcraft-cli works on"), "{args:?}: {err}");
    }
}

#[test]
fn no_subcommand_shows_the_overview_and_fails() {
    assert!(stderr(&cli(&[])).starts_with("deckcraft-cli works on"));
}

#[test]
fn help_lists_every_subcommand_with_a_run_example() {
    for flag in ["help", "--help", "-h"] {
        let out = stdout(&cli(&[flag]));
        for sub in SUBCOMMANDS {
            assert!(out.contains(&format!("\n  {sub} ")), "{flag} lacks {sub}: {out}");
        }
        assert!(out.contains("deckcraft-cli run --cmd 'slide.new={\"title\":\"Q3 results\"}' --save q3.deckcraft"), "{out}");
    }
}

#[test]
fn every_subcommand_answers_help_and_does_nothing_else() {
    let dir = std::env::temp_dir().join(format!("dc-cli-help-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for sub in SUBCOMMANDS {
        for args in [vec![sub, "--help"], vec![sub, "-h", "out.png"], vec!["help", sub]] {
            let o = Command::new(env!("CARGO_BIN_EXE_deckcraft-cli")).current_dir(&dir).args(&args).output().unwrap();
            assert!(stdout(&o).starts_with(&format!("usage: deckcraft-cli {sub}")), "{args:?}");
        }
    }
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "help wrote nothing");
    std::fs::remove_dir_all(&dir).unwrap();
    for args in [["help", "version"], ["--version", "--help"]] {
        assert!(stdout(&cli(&args)).starts_with("usage: deckcraft-cli --version"), "{args:?}");
    }
    for args in [["help", "help"], ["help", "--help"]] {
        assert!(stdout(&cli(&args)).starts_with("deckcraft-cli works on"), "{args:?}");
    }
}

#[test]
fn run_help_says_what_the_ids_do_not() {
    let out = words(&stdout(&cli(&["run", "--help"])));
    for fact in [
        "COMMAND is a command id, then `=`, then the command's parameters as one JSON object.",
        "points on a 960×540 slide",
        "Slide indices start at 0.",
        "Shape ids are looked up on the current slide only.",
        "A command that adds a shape (shape.insert, insert.textBox, insert.table…) selects it.",
        "format.* acts on the selected text or shapes",
    ] {
        assert!(out.contains(fact), "{fact}: {out}");
    }
}

#[test]
fn describe_spells_out_a_command() {
    assert_eq!(
        stdout(&cli(&["describe", "shape.insert"])),
        "shape.insert: Shapes, in Insert ▸ Illustrations.

Parameters:
  preset          rect|roundRect|ellipse|triangle|rightArrow|star5|… (see shape.presets)
  rect            [x,y,w,h] pt
  text, optional  string
  adj, optional   [number] (preset adjustments)
  flip, optional  h|v|hv

Run it with: deckcraft-cli run --cmd 'shape.insert={…}'
"
    );
    assert_eq!(
        stdout(&cli(&["describe", "format.size"])),
        "format.size: Font Size, in Home ▸ Font.

Parameters:
  size  pt
Acts on the selected text or shapes.

Run it with: deckcraft-cli run --cmd 'format.size={…}'
"
    );
    assert!(stdout(&cli(&["describe", "edit.copy"])).contains("Answers with {text}.\n\nRun it with: deckcraft-cli run --cmd edit.copy\n"));
    assert!(stdout(&cli(&["describe", "text.set"])).contains("Note: replaces the whole text, keeping the first run's formatting\n"));
    assert_eq!(
        stdout(&cli(&["describe", "shape.setDefault"])),
        "shape.setDefault: Set as Default Shape.\n\nTakes no parameters.\nActs on the selected shapes.\n\nRun it with: deckcraft-cli run --cmd shape.setDefault\n"
    );
}

#[test]
fn describe_shows_alternative_sets_of_parameters() {
    assert!(stdout(&cli(&["describe", "slide.go"])).contains("Parameters:\n  either:\n    index\n  or:\n    id\n"));
    let remove = stdout(&cli(&["describe", "animation.remove"]));
    assert!(remove.contains("  or:\n    ids, optional  (all of the shapes' animations)\n"), "{remove}");
    assert!(remove.ends_with("Run it with: deckcraft-cli run --cmd animation.remove\n") && !remove.contains("Note:"), "{remove}");
    assert!(stdout(&cli(&["describe", "shape.move"])).contains(
        "  either:
    dx, optional   pt (relative)
    dy, optional   pt (relative)
    ids, optional
  or:
    x, optional    pt (absolute)
    y, optional    pt (absolute)
    ids, optional
"
    ));
}

#[test]
fn commands_lists_one_line_per_command() {
    assert_eq!(stdout(&cli(&["commands", "format.size"])), "format.size — Font Size: {size: pt}\n");
    assert_eq!(stdout(&cli(&["commands", "slide.next"])), "slide.next — Next Slide\n");
    assert_eq!(stdout(&cli(&["commands", "shape.presets"])), "shape.presets — Shape Gallery → [{name, label, category}]\n");
}

#[test]
fn describe_names_commands_like_an_unknown_one() {
    assert_eq!(stderr(&cli(&["describe", "new"])), "deckcraft-cli: unknown command `new`; commands named like it: file.new, slide.new\n");
}

#[test]
fn a_label_before_the_command_is_an_unknown_command() {
    assert_eq!(
        stderr(&cli(&["run", "--cmd", r#"new=file.new={"blank":true}"#])),
        "deckcraft-cli: --cmd 1 of 1 was read as command `new` with parameters `file.new={\"blank\":true}`. `new` is not a command: \
         --cmd takes COMMAND=PARAMS, a command id, then `=`, then a JSON object, e.g. --cmd 'slide.new={\"title\":\"Q3\"}'. Nothing ran.\n"
    );
}

#[test]
fn bad_json_names_the_command_and_the_text() {
    let err = stderr(&cli(&["run", "--cmd", "slide.new={}", "--cmd", r#"shape.insert={"preset":"rect",}"#]));
    assert!(
        err.starts_with(
            "deckcraft-cli: --cmd 2 of 2 was read as command `shape.insert` with parameters `{\"preset\":\"rect\",}`, which are not JSON: "
        ),
        "{err}"
    );
    assert!(err.ends_with(". `shape.insert` takes {preset: rect|roundRect|ellipse|triangle|rightArrow|star5|… (see shape.presets), rect: [x,y,w,h] pt, text?: string, adj?: [number] (preset adjustments), flip?: h|v|hv}. Nothing ran.\n"), "{err}");
}

#[test]
fn a_failing_command_says_which_and_that_nothing_was_saved() {
    let out = std::env::temp_dir().join(format!("dc-cli-{}.deckcraft", std::process::id()));
    let err = stderr(&cli(&[
        "run",
        "--cmd",
        r#"shape.insert={"preset":"rect","rect":[10,10,100,50]}"#,
        "--cmd",
        "slide.new",
        "--cmd",
        r#"shape.resize={"ids":[260],"w":50}"#,
        "--save",
        out.to_str().unwrap(),
    ]));
    assert_eq!(
        err,
        "deckcraft-cli: --cmd 3 of 3: shape 260 is on slide index 0, but shape ids are looked up on slide index 1, the one being edited; \
         slide.go {\"index\": 0} switches to slide index 0. Nothing was saved or exported.\n"
    );
    assert!(!out.exists());
}

#[test]
fn print_shows_what_each_command_did() {
    let out = stdout(&cli(&[
        "run",
        "--print",
        "--cmd",
        r#"shape.insert={"preset":"roundRect","rect":[80,80,300,120],"text":"Revenue"}"#,
        "--cmd",
        r#"format.size={"size":28}"#,
        "--cmd",
        r##"design.background={"color":"#112233"}"##,
        "--cmd",
        r#"text.edit={"at":[0,3]}"#,
    ]));
    assert_eq!(
        out,
        "shape.insert → {\"id\":260}\n\
         format.size → changed shape 260 on slide index 0\n\
         design.background → changed the background of slide index 0\n\
         text.edit → editing the text of shape 260, paragraph 0, character 3\n"
    );
}

#[test]
fn without_print_run_confirms_what_it_did() {
    let out = std::env::temp_dir().join(format!("dc-cli-ok-{}.deckcraft", std::process::id()));
    let path = out.to_str().unwrap();
    assert_eq!(stdout(&cli(&["run", "--cmd", "slide.new", "--cmd", "slide.new", "--save", path])), format!("OK: 2 commands ran, saved {path}\n"));
    std::fs::remove_file(&out).unwrap();
}

#[test]
fn an_unknown_parameter_is_refused() {
    assert_eq!(
        stderr(&cli(&["run", "--cmd", r#"shape.insert={"shape":"roundRect"}"#])),
        "deckcraft-cli: --cmd 1 of 1: `shape.insert` was given `shape`, which it doesn't take. `shape.insert` takes {preset: \
         rect|roundRect|ellipse|triangle|rightArrow|star5|… (see shape.presets), rect: [x,y,w,h] pt, text?: string, \
         adj?: [number] (preset adjustments), flip?: h|v|hv}.\n"
    );
}
