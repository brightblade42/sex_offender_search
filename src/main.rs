extern crate actix_web;
extern crate serde;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use actix_web::http::{Method, HeaderMap};
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder, Result};

use rusqlite::{params, Connection, ToSql, NO_PARAMS};
use std::env;
use std::fs;

#[derive(Deserialize)]
struct Info {
    username: String,
}

#[derive(Debug, Serialize)]
struct Offender {
    id: String,
    name: String,
    dateOfBirth: String,
    eyes: String,
    hair: String,
    height: String,
    weight: String,
    race: String,
    sex: String,
    state: String,
    aliases: serde_json::Value,
    addresses: serde_json::Value,
    offenses: serde_json::Value,
    scarsTattoos: serde_json::Value,
    photos: serde_json::Value,
}

///The we extract the json search query into
//#[derive(Extract)]
#[derive(Deserialize)]
struct SearchQuery {
    name: Option<Vec<String>>,
    dob: Option<String>,
    address: Option<String>,
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

            let mut cnt = 0;
            let mut limit = 0;
            if x.len() == 0 {
                limit = 0;
            } else {
                limit = x.len() - 1;
            }

            search_frag.push_str(" ( ");
            for n in x {
                search_frag.push_str(&format!(" name like '{}'", n));
                if cnt != limit {
                    search_frag.push_str(" or ");
                }
                cnt += 1;
            }

            search_frag.push_str(" ) ");
            //search_frag.push_str(&format!(" name like '{}'", x));
            add_op = true;
        }
        None => {
            add_op = false;
        }

    }
    if let Some(addr) = &query.address {
        if add_op {
            search_frag.push_str(" and ");
        }
        search_frag.push_str(&format!(" addresses like '{}'", addr));
        add_op = true;
    }
    if let Some(x) = &query.dob {
        if add_op {
            search_frag.push_str(" and ");
        }
        search_frag.push_str(&format!(" dateOfBirth like '{}'", x));
        add_op = true;
    }

    if let Some(states) = &query.state {
        if add_op {
            search_frag.push_str(" and ");
        }
        search_frag.push_str(" state in (");
        let mut cnt = 0;
        let mut limit = 0;
        if states.len() == 0 {
            limit = 0;
        } else {
            limit = states.len() - 1;
        }
        for st in states {
            search_frag.push_str(&format!("'{}'", st));
            if cnt != limit {
                search_frag.push_str(",");
            }
            cnt += 1;
        }
        search_frag.push_str(" )");
    }
    let search_frag = search_frag.trim_end_matches(",").to_string();
    search_frag
}

fn search_offenders(query: &SearchQuery) -> Result<Vec<Offender>, rusqlite::Error> {
    let sqlp = env::var("SQL_PATH").expect("a damn sql path env variable");
    let conn = Connection::open(sqlp).expect("Unable to open data connection");
    let mut search_vec: Vec<Offender> = Vec::new();
    let qry = format!(
        r#"Select distinct trim(id) as id,trim(name) as name,dateOfBirth,eyes,hair,height,weight,race,sex,state,
                        aliases,addresses, offenses,scarsTattoos,photos
                        from SexOffender
                        where {} {}"#,
        build_search_text(query),
        " order by state, name"
    );

    let mut stmt = conn.prepare(&qry)?;
    let mut results = stmt
        .query_map(NO_PARAMS, |row| {
            //TODO see if api lets me get these values as the type i need.

            let alias: String = row.get(10)?;
            let ad: String = row.get(11)?;
            let off: String = row.get(12)?;
            let st: String = row.get(13)?;
            let ph: String = row.get(14)?;

            let addresses = serde_json::from_str(ad.as_str()).expect("a damn alias");
            let aliases = serde_json::from_str(alias.as_str()).expect("a damn alias");
            let offenses = serde_json::from_str(off.as_str()).expect("a dam value");
            let scarsTattoos = serde_json::from_str(st.as_str()).expect("a damn alias");
            let photos = serde_json::from_str(ph.as_str()).expect("a damn photo");

            let mut offender = Offender {
                id: row.get(0)?,
                name: row.get(1)?,
                dateOfBirth: row.get(2)?,
                eyes: row.get(3)?,
                hair: row.get(4)?,
                height: row.get(5)?,
                weight: row.get(6)?,
                race: row.get(7)?,
                sex: row.get(8)?,
                state: row.get(9)?,
                aliases,
                addresses,
                offenses,
                scarsTattoos,
                photos,
            };

            Ok(offender)
        })
        .expect("result row not to break");

    for r in results {
        match r {
            Ok(off) => {
                search_vec.push(off);
            },
            Err(e) => {
                println!("record go booom {}", e);
            }
        }
        //search_vec.push(r.unwrap());
    }
    Ok(search_vec)
}

///converts the Json request body into a struct
///that we use to build a search query. We run that query
///and return the results as a json document.
fn search(info: web::Json<SearchQuery>) -> HttpResponse {

    let tq = info.into_inner();
    let rez = search_offenders(&tq).expect("my dam results");
    let rezcount = rez.len();

    let jr = json!({
        "totalHits": rezcount,
        "maxPageResults": "nolimit",
        "currentPage": rezcount,
        "results": rez,
    });

    HttpResponse::Ok().content_type("application/json").json(jr)
}

///Search results contain zero or more photo names that can be
///passed back to the server to retrieve the actual image data.
///An HttpResponse with an image content type is returned.
fn get_photo(info: web::Path<(String,String)>) -> HttpResponse {
    let sqlp = env::var("SQL_PATH").expect("a damn sql path env variable");
    let photo_name = &info.1;
   let state = &info.0;

    let conn = Connection::open(sqlp).expect("Unable to open data connection");
    let mut photo: Vec<u8> = Vec::new();

    let qry = format!("Select data from photos where name='{}' and state='{}'", photo_name, state);
    let mut stmt = conn.prepare(&qry).expect("my damn prepared query");

    let mut results = stmt
        .query_map(NO_PARAMS, |row| {
            let bts: Vec<u8> = row.get(0)?;
            Ok(bts)
        })
        .expect("My damn bytes");

    for x in results {
        photo = x.unwrap();
    }

    HttpResponse::Ok().content_type("image/jpg").body(photo)
}

use actix_files::NamedFile;

fn docs(req: web::HttpRequest) -> NamedFile {

    NamedFile::open("./docs/sex_offender_search.html").expect("my dam file")
}

fn main() -> std::io::Result<()> {
    //env::set_var("SQL_PATH","/opt/eyemetric/sex_offender/app/sexoffenders.sqlite");
    env::set_var("SQL_PATH","/media/d-rezzer/data/dev/eyemetric/sex_offender/app/sexoffenders.sqlite");
    HttpServer::new(|| {
        App::new()
            .service(web::resource("/search").to(search))
            .service(web::resource("/photo/{state}/{name}").to(get_photo))
            .service(web::resource("/docs").to(docs))
    })
    .bind("0.0.0.0:8090")
    .expect("Unable to start search server")
    .run()

    // let _ = sys.run();
}
