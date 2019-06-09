extern crate actix_web;
extern crate serde;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use actix;
use actix_web::{server::HttpServer, App, HttpRequest, HttpResponse, Error, Responder, http, error, Json, Result, Path};

use actix_web::http::Method;
use bytes;
use rusqlite::{Connection, params, NO_PARAMS, ToSql};
use std::thread::JoinHandle;

static SQL_PATH: &'static str = "/home/d-rezzer/code/eyemetric/ftp/sexoffenders.sqlite";

#[derive(Deserialize)]
struct Info {
    username: String,
}

#[derive(Debug, Serialize)]
struct Offender {
    id: u32,
    name: String,
    dateOfBirth: String,
    age: String,
    addresses: serde_json::Value,
    offenses: serde_json::Value,
    aliases: serde_json::Value ,
    personalDetails: serde_json::Value,
    photos: serde_json::Value,
}

///The we extract the json search query into
//#[derive(Extract)]
#[derive(Deserialize)]
struct SearchQuery {
    name: Option<String>,
    firstName: Option<String>,
    lastName: Option<String>,
    dob: Option<String>,
    state: Option<Vec<String>>,
}

///Builds the search portion of the search query.
///This fits the search requirements but it would be nice
///come back and make this more robust.
///
///
fn build_search_text(query: &SearchQuery) -> String {
    let mut search_frag = String::new();
    let mut add_op = false;

    match &query.name {
        Some(x) => {
            search_frag.push_str(&format!(" name like '{}'", x));
            add_op = true;
        }
        None => {
            if let Some(x) = &query.firstName {
                search_frag.push_str(&format!(" name like '{}'", x));
                add_op = true;
            }
            if let Some(x) = &query.lastName {
                if add_op {
                    search_frag.push_str(" and ");
                }
                search_frag.push_str(&format!(" name like '{}'", x));
            }
        }
    }

    if let Some(x) = &query.dob {
        if add_op {
            search_frag.push_str(" and ");
        }
        search_frag.push_str(&format!(" dateOfBirth like '{}'", x));
    }
    search_frag
}

//deserialize Info from requests body
fn index(info: Json<Info>) -> Result<String> {
    Ok(format!("Welcome {}!", info.username))
}

fn search_offenders(query: &SearchQuery) -> Result<Vec<Offender>, rusqlite::Error> {
    //let sql_path  = db_path()?;


    let conn = Connection::open(SQL_PATH).expect("Unable to open data connection");
    let mut search_vec: Vec<Offender> = Vec::new();

    let qry = format!(r#"Select id,name,addresses,dateOfBirth,age,
                        offenses,aliases,personalDetails,photos
                        from SexOffender
                        where {}"#, build_search_text(query));

    let mut stmt = conn.prepare(&qry)?;
    let mut results = stmt.query_map(NO_PARAMS, |row| {

        //TODO see if api lets me get these values as the type i need.
        let ad: String = row.get(2)?;
        let off:String = row.get(5)?;
        let alias:String = row.get(6)?;
        let pd:String = row.get(7)?;
        let ph:String = row.get(8)?;
        let addresses = serde_json::from_str(ad.as_str()).expect("a damn alias");
        let offenses = serde_json::from_str(off.as_str()).expect("a dam value");
        let aliases = serde_json::from_str(alias.as_str()).expect("a damn alias");
        let personalDetails = serde_json::from_str(pd.as_str()).expect("a damn alias");
        let photos = serde_json::from_str(ph.as_str()).expect("a damn photo");
        let mut offender = Offender {
            id: row.get(0)?,
            name: row.get(1)?,
            addresses,// row.get(2)?,//json!(addr),
            dateOfBirth: row.get(3).unwrap_or(String::from("")),
            age: row.get(4).unwrap_or(String::from("")),
            offenses,
            aliases,
            personalDetails,
            photos,
        };

        Ok(offender)
    }).expect("result row not to break");

    for r in results {
        search_vec.push(r.unwrap());
    }
    Ok(search_vec)
}

fn search(info: Json<SearchQuery>) -> HttpResponse {
    let tq = info.into_inner();
    let rez = search_offenders(&tq).expect("my dam results");

    let rezcount = rez.len();

    let jr = json!({
        "totalHits": rezcount,
        "maxPageResults": "nolimit",
        "currentPage": rezcount,
        "results": rez,
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .json(jr)
}

fn get_photo(info: Path<String>) -> HttpResponse {

    let photo_name = info;
    let conn = Connection::open(SQL_PATH).expect("Unable to open data connection");
    let mut photo: Vec<u8> = Vec::new();

    let qry = format!("Select data from photos where name='{}'", photo_name);
    let mut stmt  = conn.prepare(&qry).expect("my damn prepared query");

    let mut results =  stmt.query_map(NO_PARAMS, |row| {
        let bts: Vec<u8> = row.get(0)?;
        Ok(bts)
    }).expect("My damn bytes");

    for x in results {
        photo = x.unwrap();
        println!("Ya fookn gobshite ya");
    }

    HttpResponse::Ok()
        .content_type("image/jpg")
        .body(photo)
}


fn main() {
    let sys = actix::System::new("example");
    actix_web::server::new(|| App::new()
                               .resource(
                                       "/search",
                                       |r| r.method(http::Method::POST).with(search))
        .resource("/photo/{name}", |r| r.method(http::Method::GET).with(get_photo))
    ).bind("127.0.0.1:8090")
        .expect("my damn server to run.")
        .start();

    let _ = sys.run();
}
