#[macro_use]
extern crate rocket;

use rocket::fairing::AdHoc;
use rocket::form::{Form, FromForm};
use rocket::fs::{relative, FileServer, TempFile};
use rocket::response::Redirect;
use rocket::serde::{json, Deserialize, Serialize};
use rocket::tokio::sync::Mutex;
use rocket::{Config, State};
use rocket_dyn_templates::{context, Template};
use std::fs;
use std::fs::{remove_file, File};
use std::io::{Read, Write};
use uuid::Uuid;

const DB_PATH: &str = "store/db.json";

#[get("/")]
async fn index(list: Records<'_>) -> Template {
    let list = list.lock().await;

    let mut items: Vec<&Record> = list.iter().map(|x| x).collect();
    items.reverse();

    Template::render(
        "index",
        context! {
            title: "首页" ,
            items,
        },
    )
}

#[get("/add")]
fn add_page() -> Template {
    Template::render("add", context! { title: "添加" })
}

#[derive(Debug, FromForm)]
struct AddForm<'v> {
    name: String,
    jack_board_type: String,
    jack_board_info: String,
    jack_board_imgs: Vec<TempFile<'v>>,
    equipment_info: String,
    equipment_imgs: Vec<TempFile<'v>>,
}

#[post("/", data = "<form>")]
async fn add(list: Records<'_>, mut form: Form<AddForm<'_>>) -> Redirect {
    let mut list = list.lock().await;

    let id = Uuid::new_v4().to_string();

    // 序列化保存起来
    let mut jack_board_imgs: Vec<String> = Vec::new();
    for jack_board_img in &mut form.jack_board_imgs {
        if jack_board_img.len() == 0 {
            break;
        }
        let filename = format!(
            "{}-{}",
            &id,
            jack_board_img
                .raw_name()
                .unwrap()
                .dangerous_unsafe_unsanitized_raw(),
        );

        match jack_board_img
            .persist_to(format!("static/images/{}", filename))
            .await
        {
            Ok(_) => {
                jack_board_imgs.push(format!("images/{}", filename));
            }
            Err(e) => {
                println!("{}", e);
            }
        };
    }

    let mut equipment_imgs: Vec<String> = Vec::new();
    for equipment_img in &mut form.equipment_imgs {
        if equipment_img.len() == 0 {
            break;
        }
        let filename = format!(
            "{}-{}",
            &id,
            equipment_img
                .raw_name()
                .unwrap()
                .dangerous_unsafe_unsanitized_raw(),
        );

        match equipment_img
            .persist_to(format!("static/images/{}", filename))
            .await
        {
            Ok(_) => {
                equipment_imgs.push(format!("images/{}", filename));
            }
            Err(e) => {
                println!("{}", e);
            }
        };
    }

    let record = Record {
        id,
        name: String::from(&form.name),
        jack_board_type: String::from(&form.jack_board_type),
        jack_board_info: String::from(&form.jack_board_info),
        jack_board_imgs,
        equipment_info: String::from(&form.equipment_info),
        equipment_imgs,
    };

    println!("record: {:?}", record);
    list.push(record);

    let items: Vec<&Record> = list.iter().map(|x| x).collect();
    save_in_db_json(&items);

    Redirect::to(uri!("/"))
}

#[delete("/<id>")]
async fn remove(list: Records<'_>, id: String) -> &'static str {
    let mut list = list.lock().await;
    match list.iter().find(|i| i.id == id) {
        Some(record) => {
            for path in &record.jack_board_imgs {
                if let Err(e) = remove_file(format!("static/{}", path)) {
                    println!("{}", e);
                };
            }
            for path in &record.equipment_imgs {
                if let Err(e) = remove_file(format!("static/{}", path)) {
                    println!("{}", e);
                };
            }
        }
        None => return "failed",
    };

    list.retain(|value| value.id != id);
    let items: Vec<&Record> = list.iter().map(|x| x).collect();
    save_in_db_json(&items);

    "success!"
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct Record {
    id: String,
    name: String,
    jack_board_type: String,
    jack_board_info: String,
    jack_board_imgs: Vec<String>,
    equipment_info: String,
    equipment_imgs: Vec<String>,
}

type RecordList = Mutex<Vec<Record>>;
type Records<'r> = &'r State<RecordList>;

#[launch]
fn rocket() -> _ {
    if let Err(e) = fs::create_dir_all("store") {
        println!("{}", e);
    };
    if let Err(e) = fs::create_dir_all("static/images") {
        println!("{}", e);
    };
    let file = File::open(DB_PATH);
    let mut data = String::new();
    match file {
        Ok(mut file) => {
            file.read_to_string(&mut data).unwrap();
        }
        Err(_) => {
            let mut file = File::create(DB_PATH).unwrap();
            file.write(b"[]").unwrap();
            data = "[]".to_string();
        }
    }
    println!("data: {}", data);
    let list: Vec<Record> = json::from_str(&data).unwrap();
    println!("list: {:?}", list);

    rocket::build()
        .mount("/", routes![index, add_page, add, remove])
        .attach(AdHoc::config::<Config>())
        .mount("/", FileServer::from(relative!("static")))
        .attach(Template::fairing())
        .manage(RecordList::new(list))
}

fn save_in_db_json(content: &Vec<&Record>) {
    let mut file = File::create(DB_PATH).unwrap();
    let content = json::to_string(content).unwrap();
    file.write_all(content.as_bytes()).unwrap();
}
