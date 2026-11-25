//! Tests of the target list built from the network targets of a cloud route.

use super::*;

pub(crate) const EINST: &str = r#"{"fassung":2,"klassen":["oeffentlich","intern","kundendaten"],"vorgabestufe":"kundendaten","routen":[{"schluessel":"cloud-beispiel","art":"cloud","klasse":"intern","netzziele":[{"host":"api.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":true,"pfade":[{"methode":"GET","pfad":"/copilot_internal/*"},{"methode":"POST","pfad":"/chat"}]},{"host":"telemetry.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false,"pfade":[{"methode":"POST","pfad":"/telemetry"}]},{"host":"*.frei.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false,"pfade":[{"methode":"GET","pfad":"/*"}]},{"host":"lesen.example","port":80,"zugriff":"http-lesen"}]},{"schluessel":"lokal-beispiel","art":"lokal","klasse":"kundendaten","ziel":"http://127.0.0.1:11434/v1","modell":"qwen3:8b","netzziele":[]}],"kunden":[{"schluessel":"k-beispiel","erlaubt":["lokal-beispiel"],"verboten":[]}],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]}}"#;

fn einst() -> einstellungen::Einstellungen {
    einstellungen::lesen(EINST.as_bytes()).expect("valid settings")
}

#[test]
fn only_plain_targets_with_kopf_einsetzen_enter_the_list() {
    let e = einst();
    let z = ziele(e.route("cloud-beispiel").expect("route"));
    assert_eq!(
        z,
        vec![
            Ziel {
                name: "ziel-1".to_owned(),
                host: "api.cloud.example".to_owned(),
                port: 443,
                gebunden: true,
                regeln: vec![
                    ("GET".to_owned(), "/copilot_internal/*".to_owned()),
                    ("POST".to_owned(), "/chat".to_owned())
                ],
            },
            Ziel {
                name: "ziel-2".to_owned(),
                host: "telemetry.cloud.example".to_owned(),
                port: 443,
                gebunden: false,
                regeln: vec![("POST".to_owned(), "/telemetry".to_owned())],
            },
        ]
    );
    assert_eq!(ziele(e.route("lokal-beispiel").expect("route")), Vec::new());
}

#[test]
fn the_text_names_the_holder_target_only_at_bound_targets() {
    let e = einst();
    let z = ziele(e.route("cloud-beispiel").expect("route"));
    let t = text(&z, "zugelassen");
    // No key file, no header and no value prefix: the key of a bound target lies with the holder.
    for weg in [
        "schluessel_datei",
        "wert_vorsatz",
        "authorization",
        "Bearer",
    ] {
        assert!(!t.contains(weg), "{weg} in {t}");
    }
    assert_eq!(
        t,
        "{\"fassung\":1,\"ca_zulassung\":\"zugelassen\",\"ziele\":[{\"name\":\"ziel-1\",\"host\":\"api.cloud.example\",\"port\":443,\"halter_ziel\":\"api.cloud.example\",\"regeln\":[{\"methode\":\"GET\",\"pfad\":\"/copilot_internal/*\"},{\"methode\":\"POST\",\"pfad\":\"/chat\"}],\"upgrade\":\"durchreichen\"},{\"name\":\"ziel-2\",\"host\":\"telemetry.cloud.example\",\"port\":443,\"halter_ziel\":null,\"regeln\":[{\"methode\":\"POST\",\"pfad\":\"/telemetry\"}],\"upgrade\":\"ablehnen\"}]}\n"
    );
}

/// The model path reads the list with its own loader (contract 8, section 12): no stage refuses it.
#[cfg(target_os = "linux")]
#[test]
fn the_model_path_accepts_the_list() {
    let e = einst();
    let z = ziele(e.route("cloud-beispiel").expect("route"));
    for zulassung in ["zugelassen", "offen", "abgelehnt"] {
        let t = text(&z, zulassung);
        let liste = modellweg::betriebsart::zielliste::lies(t.as_bytes())
            .expect("the model path takes the list");
        assert_eq!(liste.ziele.len(), 2);
    }
}

#[test]
fn only_a_subdomain_target_with_kopf_einsetzen_stops_the_start() {
    let e = einst();
    // the example route has `*.frei.example` with `kopf-einsetzen`
    assert!(hat_unterdomaene(e.route("cloud-beispiel").expect("route")));
    assert!(!hat_unterdomaene(e.route("lokal-beispiel").expect("route")));
    let ohne = EINST.replace("\"host\":\"*.frei.example\"", "\"host\":\"frei.example\"");
    let e = einstellungen::lesen(ohne.as_bytes()).expect("valid settings");
    assert!(!hat_unterdomaene(e.route("cloud-beispiel").expect("route")));
    // a subdomain target of another kind stays as it is: not in the list, no stop
    let lesen = EINST.replace(
        "{\"host\":\"*.frei.example\",\"port\":443,\"zugriff\":\"kopf-einsetzen\",\"schluessel\":false,\"pfade\":[{\"methode\":\"GET\",\"pfad\":\"/*\"}]}",
        "{\"host\":\"*.frei.example\",\"port\":80,\"zugriff\":\"http-lesen\"}",
    );
    assert_ne!(lesen, EINST);
    let e = einstellungen::lesen(lesen.as_bytes()).expect("valid settings");
    let route = e.route("cloud-beispiel").expect("route");
    assert!(!hat_unterdomaene(route));
    assert_eq!(ziele(route).len(), 2);
}
