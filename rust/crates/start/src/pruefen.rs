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
    if let Some(effort) = start.effort()
        && !einstellungen
            .efforts(start.harness(), start.route(), start.modell())
            .contains(&effort)
    {
        return Err(Fehler::EffortGrenze);
    }
    Ok(art)
}
