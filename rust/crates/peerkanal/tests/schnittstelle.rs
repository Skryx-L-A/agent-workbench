//! The interface of the decisions of 25.11.2026 (own SID, read time limit, client without the hello, event before the end,
//! end before 4 bytes, own bytes arrive before the end). Outside Windows: the answers of a system without the Windows side.
//! Under Windows: real pipes of this process, so the test program runs in the Windows VM (server and client are the same
//! account; the server allows its own SID without a package SID).

#[cfg(not(windows))]
mod ohne_windows {
    use std::time::Duration;

    use peerkanal::{ClientVorgabe, Code, Erwartet, Kennung, eigene_sid, verbinden_ohne_hallo};

    #[test]
    fn eigene_sid_ist_nicht_unterstuetzt() {
        let f = eigene_sid().expect_err("kein Windows");
        assert_eq!((f.code, f.os_fehler), (Code::NichtUnterstuetzt, None));
        assert_eq!(f.code.als_str(), "pk.nicht_unterstuetzt");
    }

    #[test]
    fn ohne_hallo_wie_verbinden() {
        let v = ClientVorgabe {
            name: Kennung::neu("k-beispiel").expect("form"),
            erwartet: Erwartet::Unix { uid: 501 },
            frist: Duration::from_millis(50),
            unix_ordner: Some("/tmp/pk".into()),
        };
        assert_eq!(
            verbinden_ohne_hallo(&v).err().map(|f| f.code),
            peerkanal::verbinden(&v).err().map(|f| f.code)
        );
        assert_eq!(
            verbinden_ohne_hallo(&v).err().map(|f| f.code),
            Some(Code::NichtErreichbar)
        );
    }
}

#[cfg(windows)]
#[allow(clippy::many_single_char_names)]
mod windows {
    use std::io::{ErrorKind, Read, Write};
    use std::os::windows::fs::OpenOptionsExt;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use peerkanal::{
        Annahme, ClientVorgabe, Code, Erlaubnis, Erwartet, Grund, HALLO, Kennung, Server,
        ServerVorgabe, Sid, eigene_sid, verbinden, verbinden_ohne_hallo,
    };

    /// `GENERIC_READ | FILE_WRITE_DATA`, as the library opens.
    const ZUGRIFF: u32 = 0x8000_0000 | 0x0000_0002;
    /// `SECURITY_IDENTIFICATION`.
    const IDENTIFIKATION: u32 = 0x0001_0000;

    fn name() -> Kennung {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        Kennung::neu(&format!("wb-pk-test-{}-{n}", std::process::id())).expect("form")
    }

    fn server(name: &Kennung, hallo_ms: u64) -> Server {
        let mut v = ServerVorgabe::neu(
            name.clone(),
            vec![Erlaubnis::Windows {
                sid: eigene_sid().expect("eigene SID"),
                appcontainer_sid: None,
            }],
            2,
        );
        v.hallo_frist = Duration::from_millis(hallo_ms);
        Server::anlegen(v).expect("anlegen")
    }

    fn client(name: &Kennung, erwartet: Sid) -> ClientVorgabe {
        ClientVorgabe {
            name: name.clone(),
            erwartet: Erwartet::Windows { sid: erwartet },
            frist: Duration::from_secs(2),
            unix_ordner: None,
        }
    }

    fn roh(name: &Kennung) -> std::fs::File {
        std::fs::OpenOptions::new()
            .access_mode(ZUGRIFF)
            .security_qos_flags(IDENTIFIKATION)
            .open(format!(r"\\.\pipe\{name}"))
            .expect("roh öffnen")
    }

    /// [`roh`], but waits up to 2 s while the instance is busy (`ERROR_PIPE_BUSY`): a disconnected instance takes a client only
    /// after `annehmen` ran `ConnectNamedPipe` on it again.
    fn roh_warten(name: &Kennung) -> std::fs::File {
        let ende = Instant::now() + Duration::from_secs(2);
        loop {
            match std::fs::OpenOptions::new()
                .access_mode(ZUGRIFF)
                .security_qos_flags(IDENTIFIKATION)
                .open(format!(r"\\.\pipe\{name}"))
            {
                Ok(f) => return f,
                Err(e) if e.raw_os_error() == Some(231) && Instant::now() < ende => {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(e) => panic!("roh öffnen: {e}"),
            }
        }
    }

    /// `annehmen` waits first; then `client` runs. A client that closes before `ConnectNamedPipe` would be skipped by the
    /// library (`ERROR_NO_DATA`), so the server must already wait. At most 10 s.
    fn abgewiesen_nach(s: &Arc<Server>, client: impl FnOnce()) -> peerkanal::Ereignis {
        let (tx, rx) = std::sync::mpsc::channel();
        let s2 = Arc::clone(s);
        thread::spawn(move || {
            let _ = tx.send(match s2.annehmen().expect("annehmen") {
                Annahme::Abgewiesen(e) => Some(e),
                Annahme::Vorlaeufig(_) => None,
            });
        });
        thread::sleep(Duration::from_millis(300));
        client();
        rx.recv_timeout(Duration::from_secs(10))
            .expect("annehmen kam nicht zurück")
            .expect("angenommen statt abgewiesen")
    }

    /// Decision 1: the own SID is an account SID and the owner of the own pipe.
    #[test]
    fn eigene_sid_ist_der_besitzer_der_eigenen_pipe() {
        let eigene = eigene_sid().expect("eigene SID");
        assert!(eigene.als_str().starts_with("S-1-"), "{eigene}");
        let n = name();
        let s = server(&n, 2000);
        let c = thread::spawn({
            let v = client(&n, eigene.clone());
            move || verbinden(&v).map(|v| v.peer().sid.clone())
        });
        let Annahme::Vorlaeufig(vl) = s.annehmen().expect("annehmen") else {
            panic!("abgewiesen");
        };
        assert_eq!(vl.peer().sid.as_deref(), Some(eigene.als_str()));
        // The server keeps the connection until the client has read the answer. Dropping it at once is covered by
        // `direkt_nach_bestaetigen_fallen_lassen`.
        let k = vl.bestaetigen().expect("bestätigen");
        assert_eq!(
            c.join().expect("faden").expect("verbinden"),
            Some(eigene.als_str().to_owned())
        );
        drop(k);
    }

    /// Decision 2: a silent client; the read of the server ends with `TimedOut` after the limit, and the connection goes on.
    #[test]
    fn lesefrist_der_pipe() {
        let n = name();
        let s = server(&n, 2000);
        let v = client(&n, eigene_sid().expect("eigene SID"));
        let c = thread::spawn(move || {
            let mut k = verbinden(&v).expect("verbinden");
            thread::sleep(Duration::from_millis(1500));
            k.write_all(b"x").expect("schreiben");
            thread::sleep(Duration::from_millis(500));
        });
        let Annahme::Vorlaeufig(vl) = s.annehmen().expect("annehmen") else {
            panic!("abgewiesen");
        };
        let mut k = vl.bestaetigen().expect("bestätigen");
        k.lesefrist(Some(Duration::from_millis(300)));
        let mut puffer = [0u8; 4];
        let start = Instant::now();
        let fehler = k.read(&mut puffer).expect_err("Frist");
        let dauer = start.elapsed();
        assert_eq!(fehler.kind(), ErrorKind::TimedOut);
        assert!(
            dauer >= Duration::from_millis(250) && dauer < Duration::from_millis(1400),
            "{dauer:?}"
        );
        k.lesefrist(None);
        assert_eq!(k.read(&mut puffer).expect("lesen"), 1);
        assert_eq!(puffer[0], b'x');
        c.join().expect("faden");
    }

    /// Decision 3: without the hello the client sends no byte; the server sees the end of the connection as `pk.hallo_fehlt`.
    /// The first byte of the client is its own.
    #[test]
    fn ohne_hallo_sendet_kein_pk1() {
        let n = name();
        let s = Arc::new(server(&n, 1000));
        let eigene = eigene_sid().expect("eigene SID");
        let e = abgewiesen_nach(&s, || {
            let mut k = verbinden_ohne_hallo(&client(&n, eigene.clone())).expect("verbinden");
            assert_eq!(k.peer().sid.as_deref(), Some(eigene.als_str()));
            k.write_all(b"x").expect("schreiben");
            thread::sleep(Duration::from_millis(200));
        });
        assert_eq!(e.grund, Grund::HalloFalsch, "x kam als erstes Byte");
        // Counter-check (decision 3): a wrong expected owner is `pk.falscher_server`, and the server gets no byte.
        let e = abgewiesen_nach(&s, || {
            let f = verbinden_ohne_hallo(&client(&n, Sid::dienst("wb-plattform").expect("form")))
                .err()
                .expect("falscher Server");
            assert_eq!((f.code, f.os_fehler), (Code::FalscherServer, None));
        });
        assert_eq!(e.grund, Grund::HalloFehlt, "0 Byte");
    }

    /// Decision 5 with a real pipe: open and close without a byte, and the end after `pk`, are `pk.hallo_fehlt`; a wrong byte is
    /// `pk.hallo_falsch`. Decision 4: the hook sees each event before `annehmen` returns it.
    #[test]
    fn ende_vor_vier_byte_und_haken() {
        let n = name();
        let mut s = server(&n, 2000);
        let gesehen: Arc<Mutex<Vec<String>>> = Arc::default();
        let g2 = Arc::clone(&gesehen);
        s.bei_abweisung(move |e| {
            g2.lock().expect("sperre").push(e.grund.als_text());
        });
        let s = Arc::new(s);
        for (bytes, grund) in [
            (&b""[..], Grund::HalloFehlt),
            (b"pk", Grund::HalloFehlt),
            (b"pq", Grund::HalloFalsch),
        ] {
            let e = abgewiesen_nach(&s, || {
                let mut r = roh(&n);
                r.write_all(bytes).expect("schreiben");
            });
            assert_eq!(e.grund, grund, "{bytes:?}");
            assert_eq!(
                gesehen.lock().expect("sperre").last(),
                Some(&e.grund.als_text())
            );
        }
        // The hello itself still passes. `annehmen` waits first: a disconnected instance takes no client before
        // `ConnectNamedPipe` runs on it again.
        let s2 = Arc::clone(&s);
        let a = thread::spawn(move || matches!(s2.annehmen(), Ok(Annahme::Vorlaeufig(_))));
        thread::sleep(Duration::from_millis(300));
        let mut r = roh(&n);
        r.write_all(&HALLO).expect("hallo");
        assert!(a.join().expect("faden"), "Hallo angenommen");
    }
    /// Own bytes arrive: a server that drops the connection right after `bestaetigen` still gets the hello to the client.
    /// 20 rounds with the client of the library.
    #[test]
    fn direkt_nach_bestaetigen_fallen_lassen() {
        let eigene = eigene_sid().expect("eigene SID");
        let n = name();
        let s = server(&n, 2000);
        for runde in 0..20 {
            let v = client(&n, eigene.clone());
            let c =
                thread::spawn(move || verbinden(&v).map(|_| ()).map_err(|f| (f.code, f.os_fehler)));
            let Annahme::Vorlaeufig(vl) = s.annehmen().expect("annehmen") else {
                panic!("abgewiesen in Runde {runde}");
            };
            drop(vl.bestaetigen().expect("bestätigen"));
            assert_eq!(c.join().expect("faden"), Ok(()), "Runde {runde}");
        }
    }

    /// The same with a client that reads the answer only 300 ms after its hello: the server has dropped the connection by
    /// then. Without emptying, `DisconnectNamedPipe` discards the 4 bytes.
    #[test]
    fn spaeter_leser_bekommt_pk1() {
        let n = name();
        let s = server(&n, 2000);
        for runde in 0..5 {
            let n2 = n.clone();
            let c = thread::spawn(move || {
                let mut r = roh_warten(&n2);
                r.write_all(&HALLO).expect("hallo");
                thread::sleep(Duration::from_millis(300));
                let mut puffer = [0u8; 4];
                r.read_exact(&mut puffer)
                    .map(|()| puffer)
                    .map_err(|e| e.raw_os_error())
            });
            let Annahme::Vorlaeufig(vl) = s.annehmen().expect("annehmen") else {
                panic!("abgewiesen in Runde {runde}");
            };
            drop(vl.bestaetigen().expect("bestätigen"));
            assert_eq!(c.join().expect("faden"), Ok(HALLO), "Runde {runde}");
        }
    }

    /// Counter-check: a client that never reads. The drop of the connection returns after the hello limit of the server
    /// (500 ms) plus a small span, the client sees the end instead of the hello, and the instance serves the next client.
    /// The drop runs on its own thread with a limit of 10 s, so the test fails instead of hanging.
    #[test]
    #[allow(clippy::print_stderr)]
    fn client_liest_nie_server_trennt_nach_der_frist() {
        let eigene = eigene_sid().expect("eigene SID");
        let n = name();
        let s = server(&n, 500);
        let (los, warte) = std::sync::mpsc::channel::<()>();
        let n2 = n.clone();
        let c = thread::spawn(move || {
            let mut r = roh(&n2);
            r.write_all(&HALLO).expect("hallo");
            // No read until the server has disconnected.
            let _ = warte.recv_timeout(Duration::from_secs(15));
            let mut puffer = [0u8; 4];
            r.read(&mut puffer).map_err(|e| e.raw_os_error())
        });
        let Annahme::Vorlaeufig(vl) = s.annehmen().expect("annehmen") else {
            panic!("abgewiesen");
        };
        let k = vl.bestaetigen().expect("bestätigen");
        let (fertig, dauer) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let start = Instant::now();
            drop(k);
            let _ = fertig.send(start.elapsed());
        });
        let dauer = dauer
            .recv_timeout(Duration::from_secs(10))
            .expect("das Fallenlassen blockiert über 10 s");
        eprintln!(
            "Gegenprobe: Fallenlassen nach {} ms (Frist 500 ms)",
            dauer.as_millis()
        );
        assert!(
            // The limit of 500 ms plus a span of 500 ms.
            dauer >= Duration::from_millis(450) && dauer < Duration::from_secs(1),
            "{dauer:?}"
        );
        los.send(()).expect("client");
        let gelesen = c.join().expect("faden");
        // `ERROR_PIPE_NOT_CONNECTED` (233) or the end: the hello was discarded with the connection.
        assert!(
            matches!(gelesen, Ok(0) | Err(Some(233 | 109))),
            "{gelesen:?}"
        );
        // The instance serves again.
        let v = client(&n, eigene);
        let c = thread::spawn(move || verbinden(&v).map(|_| ()).map_err(|f| f.code));
        let Annahme::Vorlaeufig(vl) = s.annehmen().expect("annehmen") else {
            panic!("abgewiesen");
        };
        let k = vl.bestaetigen().expect("bestätigen");
        assert_eq!(c.join().expect("faden"), Ok(()));
        drop(k);
    }
}
