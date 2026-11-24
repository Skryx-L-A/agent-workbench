/// The kind of a route. `cloud` sends data to a cloud service; `lokal` keeps it on the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Routenart {
    /// A cloud route.
    Cloud,
    /// A local route.
    Lokal,
}

impl Routenart {
    /// The word of the file: `cloud` or `lokal`.
    #[must_use]
    pub fn kennung(self) -> &'static str {
        match self {
            Self::Cloud => "cloud",
            Self::Lokal => "lokal",
        }
    }
}

/// The scheme of the target of a local route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schema {
    /// `http`, allowed only for `127.0.0.1`.
    Http,
    /// `https`.
    Https,
}

/// The target of a local route (version 2): scheme, host, port and path of the model base address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ziel {
    pub(crate) schema: Schema,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) pfad: String,
}

impl Ziel {
    /// The scheme.
    #[must_use]
    pub fn schema(&self) -> Schema {
        self.schema
    }

    /// The host as written: a name or an IPv4 address.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The port, 1 to 65535.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The path, ending in `/v1`.
    #[must_use]
    pub fn pfad(&self) -> &str {
        &self.pfad
    }
}

/// The access kind of a network target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zugriff {
    /// `verbinden`: a TCP tunnel without inspection.
    Verbinden,
    /// `http-lesen`: `GET` and `HEAD` over plain HTTP.
    HttpLesen,
    /// `kopf-einsetzen`: TLS ended by the model path, only the listed paths pass.
    KopfEinsetzen,
}

impl Zugriff {
    /// The word of the file.
    #[must_use]
    pub fn kennung(self) -> &'static str {
        match self {
            Self::Verbinden => "verbinden",
            Self::HttpLesen => "http-lesen",
            Self::KopfEinsetzen => "kopf-einsetzen",
        }
    }
}

/// One allowed pair of method and path pattern at a target with `kopf-einsetzen`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PfadRegel {
    pub(crate) methode: String,
    pub(crate) pfad: String,
}

impl PfadRegel {
    /// The method in capitals, such as `GET`.
    #[must_use]
    pub fn methode(&self) -> &str {
        &self.methode
    }

    /// The path pattern; it ends in `/*` when it covers everything below.
    #[must_use]
    pub fn pfad(&self) -> &str {
        &self.pfad
    }
}

/// One network target of a route (version 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Netzziel {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) zugriff: Zugriff,
    pub(crate) schluessel: bool,
    pub(crate) pfade: Vec<PfadRegel>,
    pub(crate) unterdomaene: bool,
}

impl Netzziel {
    /// The host as written; with the prefix `*.` in the subdomain form.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The port, 1 to 65535.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The access kind.
    #[must_use]
    pub fn zugriff(&self) -> Zugriff {
        self.zugriff
    }

    /// Whether the key of the route is bound to this target; `false` unless the kind is `kopf-einsetzen`.
    #[must_use]
    pub fn schluessel(&self) -> bool {
        self.schluessel
    }

    /// The allowed method and path pairs; empty unless the kind is `kopf-einsetzen`.
    #[must_use]
    pub fn pfade(&self) -> &[PfadRegel] {
        &self.pfade
    }

    /// Whether the host is in the subdomain form `*.name`.
    #[must_use]
    pub fn ist_unterdomaene(&self) -> bool {
        self.unterdomaene
    }
}

/// One route: its key, its kind and the highest data class it may see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub(crate) schluessel: String,
    pub(crate) art: Routenart,
    pub(crate) klasse: String,
    pub(crate) rang: usize,
    pub(crate) ziel: Option<Ziel>,
    pub(crate) modell: Option<String>,
    pub(crate) netzziele: Vec<Netzziel>,
}

impl Route {
    /// The key of the route.
    #[must_use]
    pub fn schluessel(&self) -> &str {
        &self.schluessel
    }

    /// The kind of the route.
    #[must_use]
    pub fn art(&self) -> Routenart {
        self.art
    }

    /// The highest data class the route may see, as named in `klassen`.
    #[must_use]
    pub fn klasse(&self) -> &str {
        &self.klasse
    }

    /// The position of the class of the route in `klassen`; the lowest class is at position 0.
    #[must_use]
    pub fn rang(&self) -> usize {
        self.rang
    }

    /// The target of a local route; `None` in version 1 and at a cloud route.
    #[must_use]
    pub fn ziel(&self) -> Option<&Ziel> {
        self.ziel.as_ref()
    }

    /// The model of the route: always set at a local route of version 2, optional at a cloud route.
    #[must_use]
    pub fn modell(&self) -> Option<&str> {
        self.modell.as_deref()
    }

    /// The network targets; empty in version 1 and at a local route.
    #[must_use]
    pub fn netzziele(&self) -> &[Netzziel] {
        &self.netzziele
    }
}

/// One customer with the routes that may and may not see the customer's data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kunde {
    pub(crate) schluessel: String,
    pub(crate) erlaubt: Vec<String>,
    pub(crate) verboten: Vec<String>,
}

impl Kunde {
    /// The key of the customer.
    #[must_use]
    pub fn schluessel(&self) -> &str {
        &self.schluessel
    }

    /// The keys of the routes that may see the customer's data; empty when the field is absent.
    #[must_use]
    pub fn erlaubt(&self) -> &[String] {
        &self.erlaubt
    }

    /// The keys of the routes that may not see the customer's data; they win over `erlaubt`.
    #[must_use]
    pub fn verboten(&self) -> &[String] {
        &self.verboten
    }
}

/// One release of a cloud route for a customer's data, as switched by an administrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Freigabe {
    pub(crate) route: String,
    pub(crate) kunde: String,
}

impl Freigabe {
    /// The key of the cloud route.
    #[must_use]
    pub fn route(&self) -> &str {
        &self.route
    }

    /// The key of the customer.
    #[must_use]
    pub fn kunde(&self) -> &str {
        &self.kunde
    }
}

/// The rule "customer data never to the cloud" with its releases. The value `vorgabe` is always `nie`
/// in fassung 1, so it is not kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KundendatenAnCloud {
    pub(crate) freigaben: Vec<Freigabe>,
}

impl KundendatenAnCloud {
    /// The releases of the required object `kundendaten_an_cloud`.
    #[must_use]
    pub fn freigaben(&self) -> &[Freigabe] {
        &self.freigaben
    }
}

/// A kind of tool that the Copilot CLI may run without asking, in a run inside the enclosure (contract 2,
/// block `harness`). The set is closed: no other value exists, so no value can mean "allow everything".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Werkzeug {
    /// `shell`
    Shell,
    /// `write`
    Write,
    /// `such-mcp`
    SuchMcp,
}

impl Werkzeug {
    /// The value of the file, which is also the argument of `--allow-tool`.
    #[must_use]
    pub fn wert(self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::Write => "write",
            Self::SuchMcp => "such-mcp",
        }
    }

    /// The tool kind of a value of the file; `None` for every other text, with no case folding.
    #[must_use]
    pub fn aus_wert(wert: &str) -> Option<Self> {
        match wert {
            "shell" => Some(Self::Shell),
            "write" => Some(Self::Write),
            "such-mcp" => Some(Self::SuchMcp),
            _ => None,
        }
    }
}

/// One server of the block `mcp` (contract 2, version 2): which of its tools read, which write, and the
/// release rule for its write calls. Contract 6 reads this view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServer {
    pub(crate) name: String,
    pub(crate) lesen: Vec<String>,
    pub(crate) schreiben: Vec<String>,
    pub(crate) freigabe: bool,
    pub(crate) frist_ms: u64,
    pub(crate) einloesefrist_ms: u64,
}

impl McpServer {
    /// The key of the server in the block `mcp`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The tools that only read, in the order of the file.
    #[must_use]
    pub fn lesen(&self) -> &[String] {
        &self.lesen
    }

    /// The tools that write, in the order of the file.
    #[must_use]
    pub fn schreiben(&self) -> &[String] {
        &self.schreiben
    }

    /// Whether each write call needs a release: `freigabe` of the file, `true` when absent.
    #[must_use]
    pub fn freigabe(&self) -> bool {
        self.freigabe
    }

    /// The longest deadline for the decision of a person in milliseconds: 60000 when absent.
    #[must_use]
    pub fn frist_ms(&self) -> u64 {
        self.frist_ms
    }

    /// The deadline to redeem a release in milliseconds: 300000 when absent.
    #[must_use]
    pub fn einloesefrist_ms(&self) -> u64 {
        self.einloesefrist_ms
    }
}

/// The typed view of a settings file that passed all stages of contract 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Einstellungen {
    pub(crate) fassung: u8,
    pub(crate) klassen: Vec<String>,
    pub(crate) vorgabe_rang: usize,
    pub(crate) routen: Vec<Route>,
    pub(crate) kunden: Vec<Kunde>,
    pub(crate) kundendaten_an_cloud: KundendatenAnCloud,
    pub(crate) copilot_ohne_rueckfrage: Vec<Werkzeug>,
    pub(crate) mcp: Vec<McpServer>,
}

impl Einstellungen {
    /// All servers of the block `mcp`, in the order of the file. Empty when the block is absent or the file
    /// has version 1: no server is released (fail closed).
    #[must_use]
    pub fn mcp(&self) -> &[McpServer] {
        &self.mcp
    }

    /// The server with the given key. `None` when the block or the server is absent: no server is released.
    #[must_use]
    pub fn mcp_server(&self, name: &str) -> Option<&McpServer> {
        self.mcp.iter().find(|server| server.name == name)
    }

    /// The version of the file: 1 or 2.
    #[must_use]
    pub fn fassung(&self) -> u8 {
        self.fassung
    }

    /// The data classes, the lowest first. The last one is the highest class, which counts as customer data.
    #[must_use]
    pub fn klassen(&self) -> &[String] {
        &self.klassen
    }

    /// The class of a unit without a class: the stated `vorgabestufe`, or the highest class if the field is absent.
    #[must_use]
    pub fn vorgabestufe(&self) -> &str {
        self.klassen
            .get(self.vorgabe_rang)
            .map_or("", String::as_str)
    }

    /// The position of the class of a unit without a class in `klassen`.
    #[must_use]
    pub fn vorgabestufe_rang(&self) -> usize {
        self.vorgabe_rang
    }

    /// The tool kinds that the Copilot CLI runs without asking, in the order of the file
    /// (`harness.copilot.werkzeuge_ohne_rueckfrage`). Empty when the block is absent or the list is empty:
    /// the question of the CLI stays.
    #[must_use]
    pub fn copilot_ohne_rueckfrage(&self) -> &[Werkzeug] {
        &self.copilot_ohne_rueckfrage
    }

    /// All routes, in the order of the file.
    #[must_use]
    pub fn routen(&self) -> &[Route] {
        &self.routen
    }

    /// The route with the given key, if the file has one.
    #[must_use]
    pub fn route(&self, schluessel: &str) -> Option<&Route> {
        self.routen
            .iter()
            .find(|route| route.schluessel == schluessel)
    }

    /// All customers, in the order of the file.
    #[must_use]
    pub fn kunden(&self) -> &[Kunde] {
        &self.kunden
    }

    /// The customer with the given key, if the file has one.
    #[must_use]
    pub fn kunde(&self, schluessel: &str) -> Option<&Kunde> {
        self.kunden
            .iter()
            .find(|kunde| kunde.schluessel == schluessel)
    }

    /// The rule "customer data never to the cloud" with its releases.
    #[must_use]
    pub fn kundendaten_an_cloud(&self) -> &KundendatenAnCloud {
        &self.kundendaten_an_cloud
    }
}
