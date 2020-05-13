extern crate actix_web;
extern crate serde;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use actix_web::http::{Method, HeaderMap};
use actix_web::{web, App, get, HttpRequest, HttpResponse, HttpServer, Responder, Result};

use rusqlite::{params, Connection, ToSql, NO_PARAMS};
use std::env;
use std::fs;

use actix_files::NamedFile;
use actix_web::client;
use actix_web::error::ParseError::Uri;
use user_pass::user_account::UserAccount;
use base64::display::Base64Display;

static SXOFF_DB: &'static str = "SXOFF_DB";

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

///check user/pass against local auth db.
 async fn validate_request(req: &HttpRequest) -> bool {

    match req.headers().get("Authorization") {
        Some(auth) => {

            let token = auth.to_str().unwrap();
            let b64str = &token[6..];
            let decoded = base64::decode(&token[6..]).unwrap();
            let orig = String::from_utf8(decoded).unwrap();
            let upv: Vec<&str> = orig.split(":").collect();
            let user = upv[0];
            let pass = upv[1];
            let is_verified = UserAccount::verify(user, pass).unwrap_or_else(|_| false);
            if !is_verified { return false }; //short circuit.
            let is_active = UserAccount::is_active(user);

            is_verified && is_active
        },
        None => false //no auth token, not valid
    }

}
///converts the Json request body into a struct
///that we use to build a search query. We run that query
///and return the results as a json document.
async fn search(req: HttpRequest, info: web::Json<SearchQuery>) -> impl Responder {

    if !validate_request(&req).await {
       return HttpResponse::Unauthorized().finish();
    }

    let results = search_offenders(&info.into_inner()).await.expect("Unable to get results"); //.expect("my dam results").await;
    let result_count = results.len();

    let jr = json!({
        "totalHits": result_count,
        "maxPageResults": "nolimit",
        "currentPage": result_count,
        "results": results,
    });

    HttpResponse::Ok().content_type("application/json").json(jr)
}

///Search results contain zero or more photo names that can be
///passed back to the server to retrieve the actual image data.
///An HttpResponse with an image content type is returned.
async fn get_photo(req: HttpRequest, info: web::Path<(String,String)>) -> HttpResponse {

    if !validate_request(&req).await {
        return HttpResponse::Unauthorized().finish();
    }

    let sqlp = env::var(SXOFF_DB).expect("SQL_PATH Env var not set");
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


async fn docs(req: web::HttpRequest) -> Result<NamedFile, std::io::Error> {
    NamedFile::open("./docs/sex_offender_search.html")
}



///Builds the search portion of the search query.
///This fits the search requirements but it would be nice
///come back and make this more robust.
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

async fn search_offenders(query: &SearchQuery) -> Result<Vec<Offender>, rusqlite::Error> {
    let sqlp = env::var(SXOFF_DB).expect("a damn sql path env variable");
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

            let aliases: String = row.get(10)?;
            let addresses: String = row.get(11)?;
            let offenses: String = row.get(12)?;
            let scars_tats: String = row.get(13)?;
            let photos: String = row.get(14)?;

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
                aliases: serde_json::from_str(aliases.as_str()).expect("no alias found"),
                addresses: serde_json::from_str(addresses.as_str()).expect("a damn alias"),
                offenses: serde_json::from_str(offenses.as_str()).expect("no offense column"),
                scarsTattoos: serde_json::from_str(scars_tats.as_str()).expect("a damn alias"),
                photos: serde_json::from_str(photos.as_str()).expect("a damn photo"),
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


#[actix_rt::main]
async fn main() -> std::io::Result<()> {

    env::set_var("AUTH_DB","/opt/eyemetric/sex_offender/app/auth.db");
    env::set_var("SXOFF_DB","/opt/eyemetric/sex_offender/app/sexoffenders.sqlite");
    HttpServer::new(|| {
        App::new()
            .route("/search", web::post().to(search))
            .route("/docs", web::get().to(docs))
            .route("/photo/{state}/{name}", web::get().to(get_photo))
    })
        .bind("0.0.0.0:8090")
        .expect("Unable to start search server")
        .run().await


}
