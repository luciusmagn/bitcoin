/// prohledávač bitcoinových adresu dle zadání.
/// Pokud je program součástí pipeline (!atty), vypíše výstup do stdout,
/// jinak se zeptá uživatele
///
/// ### Kompilace:
/// ```bash
/// cargo build --release
/// ```
/// Binární soubor bude target/release/odmociny
///
/// ### Spouštení přes Cargo
/// ```bash
/// cargo run --release
/// ```
///
/// moje prostředí: Arch Linux x86_64, GCC, Rust 1.34.0-nightly (c1d2d83ca 2019-03-01)
extern crate reqwest;
extern crate serde;
extern crate serde_derive;
extern crate promptly;
extern crate yansi;
extern crate chrono;
extern crate atty;

use yansi::Paint;
use promptly::{prompt, prompt_default};
use serde_derive::{Deserialize};
use atty::Stream;

use std::process::exit;
use std::io::{Write};
use std::fs::File;

/// Příjemce transakce
#[derive(Deserialize, Clone, PartialEq, Eq)]
struct Receiver {
    addr: Option<String>,
    value: i64,
}

/// Struktura pro transakci
#[derive(Deserialize, Clone, PartialEq, Eq)]
struct Transakce {
    out: Vec<Receiver>,
    time: u64,
}

/// Struktura pro adresu vrácenou z online API
#[derive(Deserialize, Clone, PartialEq, Eq)]
struct BtcAddr {
    txs: Vec<Transakce>,
    final_balance: i64,
    address: String,
}

/// Stáhne JSON dané adresy a naparsuje jí do struktur
fn download_addr_json(a: &str) -> Result<BtcAddr, String> {
    // nemůžeme-li se připojit k serveru, je vše ztraceno
    let mut req = reqwest::get(&format!("https://blockchain.info/rawaddr/{}", a))
        .unwrap_or_else(|_| {
            eprintln!("Nepodařilo se odeslat požadavek na blockchain.info");
            exit(-1)
        });

    if !(200 <= req.status().as_u16() && req.status().as_u16() < 300) {
        return Err(format!("nepodařilo se správně přečíst data, server odpověděl s kódem {}", req.status()));
    }

    let body = match req.text() {
        Err(_) => return Err("Nepodařilo se přečíst tělo odpovědi serveru jako text".to_string()),
        Ok(b) => b,
    };

    match serde_json::from_str(&body) {
        Err(e) => Err(format!("nepodařilo se naparsovat přijatá data {0} {0:?}", e)),
        Ok(d) => Ok(d),
    }
}

fn main() {
    let adresa: String = prompt(format!("{}", Paint::yellow("Zadejte počáteční adresu")));
    let uroven: u32 = prompt(format!("{}", Paint::yellow("Zadejte do kolikáté úrovně projíždět transakce")));
    let cas: String = prompt_default(format!("{}", Paint::yellow("Zadejte čas od kterého chcete vypisovat transakce ve formátu -> %Y-%m-%d %H:%M:%S <-")), "1970-01-01 00:00:00".to_string());

    let uroven = uroven as usize;
    let cas = chrono::NaiveDateTime::parse_from_str(&cas, "%Y-%m-%d %H:%M:%S").unwrap_or_else(|_| {
        eprintln!("cas je ve špatném formátu");
        exit(-1);
    });
    let cas = cas.timestamp() as u64;

    // výstup
    let mut out = String::new();
    out.push_str(&adresa);
    out.push('\n');

    // pokud se nepodaří stáhnout první adresu, tak to nemá smysl
    let initial = match download_addr_json(&adresa) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("došlo k chybě: {}", e);
            exit(-1)
        },
    };

    // další úroveň
    let mut next_addrs = vec![initial];

    // projde úrovně
    (0..uroven).for_each(|u| {
        let current_addrs = next_addrs.clone();
        next_addrs.clear();

        out.push_str(&format!("{}. úroveň\n", u+1));

        for mut current in current_addrs {
            // vyhodíme všechny transakce starší než zadaný čas
            current.txs.retain(|t| t.time > cas);

            // jedna transakce může být patrně na více adres, proto je potřeba je všechny projít
            for outs in current.txs.iter().map(|x| x.out.clone()) {
                outs.iter()
                    .filter(|r| r.addr.is_some())
                    .inspect(|r| out.push_str(&format!("{}, {}", r.addr.clone().unwrap(), r.value)))
                    .map(|r| r.addr.clone().unwrap().to_string())
                    .map(|a| download_addr_json(&a))
                    .filter(|a| a.is_ok())
                    .for_each(|a| next_addrs.push(a.unwrap()))
            }
        }

        next_addrs.dedup()
    });

    if prompt_default("Vypsat výstup do standardního výstupu?", true) || !atty::is(Stream::Stdout) {
        println!("{}", out)
    } else {
        let filename: String = prompt_default("Název souboru", "out.csv".to_string());
        let mut file = match File::create(&filename) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("došlo k chybě při vytváření souboru: {}", e);
                exit(-1)
            },
        };

        match write!(file, "{}", out) {
            Ok(_) => eprintln!("data byla zapsána v pořádku"),
            Err(e) => {
                eprintln!("došlo k chybě při zápisu dat: {}", e);
                exit(-1)
            }
        }
        // soubory v Rustu se nemusí zavírat manuálně, viz std::fs::File
    }
}
