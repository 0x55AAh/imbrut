use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufRead, Lines};
use std::vec::IntoIter;
use std::{thread, time, env};

use itertools::{Itertools, CombinationsWithReplacement};

use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{
    self, 
    header::{HeaderMap, HeaderName, HeaderValue}, 
    blocking::RequestBuilder
};

use config;
use http;

type CheckResult = Result<(), ()>;
type Passwords = Box<dyn Iterator<Item = String>>;

trait Proto {
    fn check(&self, username: &str, password: &str) -> CheckResult;
}

struct PasswordsFile {
    items: Lines<BufReader<File>>,
}

impl PasswordsFile {
    fn new(path: &str) -> Self {
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
        Self { items: reader.lines() }
    }
}

impl Iterator for PasswordsFile {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.items.next().and_then(|r| r.ok())
    }
}

struct PasswordsGenerator {
    items: CombinationsWithReplacement<IntoIter<char>>,
}

impl PasswordsGenerator {
    fn new(allowed_chars: &Vec<String>, size: usize) -> Self {
        let allowed_chars: Vec<char> = allowed_chars.concat().chars().collect();
        let items = allowed_chars.into_iter()
            .combinations_with_replacement(size);
        Self { items }
    }
}

impl Iterator for PasswordsGenerator {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.items.next().and_then(|r| Some(r.into_iter().collect()))
    }
}

struct HTTPProto {
    auth_type: String,
    success_codes: Vec<http::StatusCode>,
    request: RequestBuilder,
    success_if_containes: Vec<String>,
    fail_if_containes: Vec<String>,
}

impl HTTPProto {
    fn new(target: &HashMap<String, config::Value>) -> Self {
        let success_codes: Vec<u16> = target.get("success_codes").unwrap().clone()
            .into_array()
            .unwrap()
            .into_iter()
            .map(|x| x.into_uint().unwrap() as u16)
            .collect();
        let success_codes = success_codes.into_iter()
            .map(|x| http::StatusCode::from_u16(x).unwrap())
            .collect();
        
        let auth_type = target.get("auth_type").unwrap().to_string();

        let success_if_containes: Vec<String> = target.get("success_if_containes").unwrap().clone()
            .into_array()
            .unwrap()
            .into_iter()
            .map(|x| x.to_string())
            .collect(); // TODO
        
        let fail_if_containes: Vec<String> = target.get("fail_if_containes").unwrap().clone()
            .into_array()
            .unwrap()
            .into_iter()
            .map(|x| x.to_string())
            .collect(); // TODO
        
        let request = Self::build_request(&target);

        Self { 
            auth_type,
            success_codes,
            request,
            success_if_containes,
            fail_if_containes,
        }
    }

    fn build_request(target: &HashMap<String, config::Value>) -> RequestBuilder {
        let uri = target.get("uri").unwrap().to_string();

        let method = target.get("method").unwrap().to_string(); // TODO: default POST
        let method = http::Method::from_bytes(method.as_bytes()).unwrap();

        let client = reqwest::blocking::Client::new();
        let mut request = client.request(method, uri);

        let _headers: HashMap<String, String> = target.get("headers").unwrap().clone() // TODO: default empty hashmap
            .into_table()
            .unwrap()
            .into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect();
        let mut headers = HeaderMap::new();
        
        for (key, value) in _headers {
            let key = HeaderName::from_bytes(key.as_bytes()).unwrap();
            let val = HeaderValue::from_bytes(value.as_bytes()).unwrap();
            headers.insert(key, val);
        }

        request = request.headers(headers);

        request
    }
}

impl Proto for HTTPProto {
    fn check(&self, username: &str, password: &str) -> CheckResult {
        let mut request = self.request.try_clone().unwrap();

        match self.auth_type.as_str() {
            "form" => {
                request = request.form(&[(username, password)]);
            }
            "basic" => {
                request = request.basic_auth(username, Some(password));
            }
            _ => {
                panic!("Unsupported authentication type: {}", self.auth_type)
            }
        }
        
        let response = request.send().unwrap();

        let responce_status = response.status();
        let responce_content = response.text().unwrap();

        if self.success_codes.contains(&responce_status) {
            for x in &self.fail_if_containes {
                if responce_content.contains(x) {
                    return Err(());
                }
            }
            for x in &self.success_if_containes {
                if responce_content.contains(x) {
                    return Ok(());
                }
            }
        }

        Err(())
    }
}

struct Settings {
    usernames_file: String,
    passwords_file: String,
    sleep: u64,
    dict_type: String,
    proto: String,
    target: HashMap<String, config::Value>,
    password_len: usize,
    allowed_chars: Vec<String>,
}

impl Settings {
    fn new() -> Self {
        let config_file = env::var("IMBRUT_CONFIG")
            .unwrap_or("config.yml".to_string());
        let passwords_file = env::var("IMBRUT_PASSWORDS_FILE")
            .unwrap_or("passwords.txt".to_string());
        let usernames_file = env::var("IMBRUT_USERNAMES_FILE")
            .unwrap_or("usernames.txt".to_string());

        let config = config::Config::builder()
            .add_source(config::File::with_name(config_file.as_str()))
            .build()
            .unwrap();  // TODO: create default config?
        
        let sleep = config.get_int("sleep").unwrap_or(0) as u64;

        let dict_type = config.get_string("dict_type")
            .unwrap_or("file".to_string())
            .to_lowercase();

        let dict_props = config.get_table("dict_props").unwrap(); // TODO
        let password_len = dict_props.get("password_length").unwrap().clone()
            .into_uint()
            .unwrap() as usize; // TODO
        let allowed_chars: Vec<String> = dict_props.get("allowed_chars").unwrap().clone()
            .into_array()
            .unwrap()
            .into_iter()
            .map(|x| x.to_string())
            .collect(); // TODO

        let proto = config.get_string("proto")
            .unwrap_or("http".to_string())
            .to_lowercase();
            
        let target = config.get_table("target").unwrap(); // TODO: raise error

        Self { 
            usernames_file,
            passwords_file,
            sleep,
            dict_type,
            proto,
            target,
            password_len,
            allowed_chars,
        }
    }

    fn save() {
        // TODO: save data into yaml file
    }
}

struct Progress { 
    pb: ProgressBar,
}

impl Progress {
    fn new(workload: usize) -> Self {
        let pb = ProgressBar::new(workload as u64);
        Self::customize(&pb);
        Self { pb }
    }

    fn customize(pb: &ProgressBar) {
        let template = "{spinner:.green} [{elapsed_precise}] {percent}% {bar:50} {human_pos} of {human_len} | ETA: {eta_precise} | {msg}";
        pb.set_style(
            ProgressStyle::with_template(template).unwrap()
            // .with_key("eta", |s, w| write!(w, "{}", s.eta().as_secs()).unwrap())
        );
    }

    fn update(&mut self, item: String) {
        let msg = format!("current: {}", item);
        self.pb.set_message(msg);
        self.pb.inc(1);
    }

    fn finish(&self, item: Option<String>) {
        if let Some(item) = item {
            let msg = format!("match: {}", item);
            self.pb.abandon_with_message(msg);
        } else {
            self.pb.abandon();
        }
    }
}


pub struct Application {
    settings: Settings,
    proto: Box<dyn Proto>,
    version: String,
}

impl Application {
    pub fn new() -> Self {
        let settings = Settings::new();
        
        let proto = match settings.proto.as_str() {
            "http" => {
                Box::new(HTTPProto::new(&settings.target))
            }
            _ => {
                panic!("Unsupported protocol: {}", settings.proto);
            }
        };

        let version = env!("CARGO_PKG_VERSION").to_string();

        Self {
            settings,
            proto,
            version,
        }
    }

    fn show_splash(&self) {
        println!("
        ██▓    ▄▄▄       ███▄ ▄███▓    ▄▄▄▄    ██▀███   █    ██ ▄▄▄█████▓
       ▓██▒   ▒████▄    ▓██▒▀█▀ ██▒   ▓█████▄ ▓██ ▒ ██▒ ██  ▓██▒▓  ██▒ ▓▒
       ▒██▒   ▒██  ▀█▄  ▓██    ▓██░   ▒██▒ ▄██▓██ ░▄█ ▒▓██  ▒██░▒ ▓██░ ▒░
       ░██░   ░██▄▄▄▄██ ▒██    ▒██    ▒██░█▀  ▒██▀▀█▄  ▓▓█  ░██░░ ▓██▓ ░ 
       ░██░    ▓█   ▓██▒▒██▒   ░██▒   ░▓█  ▀█▓░██▓ ▒██▒▒▒█████▓   ▒██▒ ░ 
       ░▓      ▒▒   ▓▒█░░ ▒░   ░  ░   ░▒▓███▀▒░ ▒▓ ░▒▓░░▒▓▒ ▒ ▒   ▒ ░░   
        ▒ ░     ▒   ▒▒ ░░  ░      ░   ▒░▒   ░   ░▒ ░ ▒░░░▒░ ░ ░     ░    
        ▒ ░     ░   ▒   ░      ░       ░    ░   ░░   ░  ░░░ ░ ░   ░      
        ░           ░  ░       ░       ░         ░        ░              
                                            ░              VERSION: {}
       ", self.version);
    }

    fn get_passwords(&self) -> (Passwords, usize) {
        match self.settings.dict_type.as_str() {
            "file" => {
                (
                    Box::new(PasswordsFile::new(&self.settings.passwords_file)),
                    PasswordsFile::new(&self.settings.passwords_file).count(),
                )
            }
            "generator" => {
                let (
                    allowed_chars, 
                    password_len
                ) = (&self.settings.allowed_chars, self.settings.password_len);
                (
                    Box::new(PasswordsGenerator::new(allowed_chars, password_len)),
                    PasswordsGenerator::new(allowed_chars, password_len).count(),
                )
            }
            _ => {
                panic!("Unsupported dict type: {}", self.settings.dict_type);
            }
        }
    }

    fn get_usernames(&self) {
        // TODO
    }

    /// Application entrypoint
    pub fn run(&self) {
        self.show_splash();

        let (passwords, workload): (Passwords, usize) =  self.get_passwords();
        // let (usernames, count): (Passwords, usize) =  self.get_usernames();
        let mut progress = Progress::new(workload);

        if self.settings.sleep > 0 {
            for password in passwords {
                // TODO: custom username
                if let Ok(_) =  self.proto.check("admin", &password) {
                    progress.finish(Some(password));
                    break;
                }
                thread::sleep(time::Duration::from_millis(self.settings.sleep));
                progress.update(password);
            }
        } else {
            for password in passwords {
                // TODO: custom username
                if let Ok(_) =  self.proto.check("admin", &password) {
                    progress.finish(Some(password));
                    break;
                }
                progress.update(password);
            }
        }

        progress.finish(None);
    }
}