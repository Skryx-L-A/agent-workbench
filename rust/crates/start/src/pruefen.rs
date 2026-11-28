use einstellungen::{Einstellungen, Routenart};

use crate::Fehler;
use crate::form::{Startform, harness_art};

/// Checks a start against the settings file: stages 14 to 18 of contract 1.
///
/// # Errors
/// Returns `start.route_unbekannt`, `start.modell_route`, `start.kunde_unbekannt`,
/// `start.harness_route_unvertraeglich` or `start.effort_grenze`, in this order, for the first violated stage.
pub fn pruefen<S: Startform>(
    start: &S,
    einstellungen: &Einstellungen,
) -> Result<Routenart, Fehler> {
    let route = einstellungen
        .route(start.route())
        .ok_or(Fehler::RouteUnbekannt)?;
    if route
        .modell()
        .is_some_and(|modell| modell != start.modell())
    {
        return Err(Fehler::ModellRoute);
    }
    if start
        .kunde()
        .is_some_and(|kunde| einstellungen.kunde(kunde).is_none())
    {
        return Err(Fehler::KundeUnbekannt);
    }
    let art = harness_art(start.harness()).ok_or(Fehler::HarnessUnbekannt)?;
    if art != route.art() {
        return Err(Fehler::HarnessRouteUnvertraeglich);
    }
    let erlaubt = einstellungen.efforts(start.harness(), start.route(), start.modell());
    let in_grenze = match gewaehlt(start, einstellungen) {
        Some(effort) => erlaubt.contains(&effort),
        // the fallback stands for a start without an effort: valid while no effort can be chosen
        None => rueckfall(start.harness())
            .is_none_or(|effort| erlaubt.is_empty() || erlaubt.contains(&effort)),
    };
    if !in_grenze {
        return Err(Fehler::EffortGrenze);
    }
    Ok(art)
}

/// The effort "Lauf starten" passes on to the harness (contract 1, "Effort immer ausdrücklich"; decision of the
/// operator, 28 November 2026): the `effort` of the start; without one the default of the admins for harness, route and
/// model (contract 2, `effort.grenzen[].vorgabe`); without that `none` for `copilot`, and `None` for every other
/// harness. Reason: without an option the Copilot CLI sends `medium` itself, so the limit must see what is really sent.
/// [`pruefen`] checks the same value in stage 18.
#[must_use]
pub fn effort<'a, S: Startform>(start: &'a S, einstellungen: &'a Einstellungen) -> Option<&'a str> {
    gewaehlt(start, einstellungen).or_else(|| rueckfall(start.harness()))
}

/// The effort of the start, or else the default of the admins.
fn gewaehlt<'a, S: Startform>(start: &'a S, einstellungen: &'a Einstellungen) -> Option<&'a str> {
    start
        .effort()
        .or_else(|| einstellungen.effort_vorgabe(start.harness(), start.route(), start.modell()))
}

/// The effort of a harness without an effort and without a default: `none` for `copilot`, whose CLI then sends no
/// effort at all (measured with 1.0.88 and 1.0.117).
fn rueckfall(harness: &str) -> Option<&'static str> {
    (harness == "copilot").then_some(COPILOT_RUECKFALL)
}

/// The effort `copilot` gets without an effort and without a default.
pub const COPILOT_RUECKFALL: &str = "none";
