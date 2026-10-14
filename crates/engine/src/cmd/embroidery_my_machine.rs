//! My Machine (E4): the embroidery machine someone sews on, chosen once. New designs take its
//! everyday hoop and its machine settings, and Save for My Machine (`embroidery.send`) writes its
//! file format to its folder or USB stick.

use serde_json::{Value, json};
use sf_core::machines::{HOME_MACHINES, HomeMachine, home_machine};

use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "embroidery.myMachine",
            "My Machine",
            [],
            None,
            "{} the machine chosen as one's own, the machines to choose from and the folder or USB stick designs were last saved to → {machine: name | null, format, hoops: [{name, width, height}] (mm, the everyday one first), folder: path | null, machines: [{name, format}]}",
            always,
            get
        ),
        cmd!(
            "embroidery.myMachine.set",
            "Set My Machine",
            [],
            None,
            "{machine: a name from embroidery.myMachine's machines | null (none)} choose one's machine: new designs take its first hoop and its settings, and Save for My Machine writes its format → as embroidery.myMachine",
            always,
            set
        ),
    ]
}

/// The machine chosen as one's own, if any.
pub(super) fn mine(s: &Session) -> Option<&'static HomeMachine> {
    Some(s.prefs.my_machine.as_str()).filter(|n| !n.is_empty()).and_then(home_machine)
}

fn get(s: &mut Session, _: &Value) -> Result<Value> {
    let m = mine(s);
    let hoops: Vec<Value> = m
        .map(|m| m.hoops)
        .unwrap_or_default()
        .iter()
        .filter_map(|name| super::embroidery_hoops::find_hoop(s, name))
        .map(|h| json!({ "name": h.name, "width": h.width, "height": h.height }))
        .collect();
    Ok(json!({
        "machine": m.map(|m| m.name),
        "format": m.map(|m| m.format),
        "hoops": hoops,
        "folder": s.prefs.embroidery_send_dirs.first(),
        "machines": HOME_MACHINES.iter().map(|m| json!({ "name": m.name, "format": m.format })).collect::<Vec<_>>(),
    }))
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.myMachine.set";
    match p.get("machine") {
        None => return Err(bad(CMD, "missing `machine`: a name from embroidery.myMachine, or null")),
        Some(Value::Null) => s.prefs.my_machine.clear(),
        Some(v) => {
            let name = v.as_str().ok_or_else(|| bad(CMD, "`machine` is a name or null"))?;
            let m = home_machine(name).ok_or_else(|| bad(CMD, format!("unknown machine `{name}`: see embroidery.myMachine")))?;
            s.prefs.my_machine = m.name.to_string();
        }
    }
    get(s, &json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// E4: the machine chosen once sets new designs' hoop and machine, and the file format saved.
    #[test]
    fn my_machine_sets_the_hoop_the_settings_and_the_format() {
        let mut s = Session::new();
        let none = s.execute("embroidery.myMachine", &json!({})).unwrap();
        assert!(none["machine"].is_null() && none["machines"].as_array().unwrap().len() >= 5);
        assert!(s.execute("embroidery.newDesign", &json!({})).is_err(), "no hoop and no machine");
        let r = s.execute("embroidery.myMachine.set", &json!({"machine": "Husqvarna Viking Designer Diamond deLuxe"})).unwrap();
        assert_eq!(r["format"], "vp3");
        assert_eq!(r["hoops"][0]["name"], "Husqvarna 240 × 150");
        // a new design without a hoop: the machine's everyday hoop, its settings
        let d = s.execute("embroidery.newDesign", &json!({})).unwrap();
        assert_eq!(d["hoop"], "Husqvarna 240 × 150");
        s.execute("shape.rectangle", &json!({"x": 100, "y": 100, "width": 100, "height": 60})).unwrap();
        s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
        let out = s.execute("embroidery.export", &json!({})).unwrap();
        assert_eq!(out["format"], "vp3", "the machine's format");
        assert!(s.execute("embroidery.myMachine.set", &json!({"machine": "Sewing Wonder 3000"})).is_err());
        s.execute("embroidery.myMachine.set", &json!({"machine": null})).unwrap();
        assert!(s.execute("embroidery.myMachine", &json!({})).unwrap()["machine"].is_null());
    }
}
