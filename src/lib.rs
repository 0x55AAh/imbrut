use config::{self, ConfigError, Value, Map};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufRead, Lines};
use std::path::Path;
use http::{Request, request};
use std::clone::Clone;
use std::{thread, result};
use std::time;
use std::env;
use indicatif::ProgressBar;

enum Result {
    MATCH,
    MISS,
}

trait Proto {
    fn check(&self, password: &str) -> Result;
}

struct PasswordsFile {
    lines: Lines<BufReader<File>>,
}

impl PasswordsFile {
    fn new(path: impl AsRef<Path>) -> Self {
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
        Self { 
            lines: reader.lines(),
        }
    }
}

impl Iterator for PasswordsFile {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        match self.lines.next() {
            Some(res) => {
                if let Ok(password) = res {
                    return Some(password);
                }
                None
            },
            None => None,
        }
    }
}

struct HTTPProto {
    success_codes: Vec<usize>,
    request: request::Builder,
}

impl HTTPProto {
    fn new(target: &HashMap<String, Value>) -> Self {
        let success_codes = target.get("success_codes").unwrap().clone()
            .into_array()
            .unwrap()
            .into_iter()
            .map(|x| x.into_uint().unwrap() as usize)
            .collect();

        Self { 
            success_codes,
            request: Self::build_request(&target),
        }
    }

    fn build_request(target: &HashMap<String, Value>) -> request::Builder {
        let uri = target.get("uri").unwrap().to_string();
        let method = target.get("method").unwrap().to_string();

        let mut request = Request::builder()
            .method(method.as_str())
            .uri(uri.as_str());

        
        let headers: HashMap<String, String> = target.get("headers").unwrap().clone()
            .into_table()
            .unwrap()
            .into_iter()
            .map(|x| (x.0, x.1.to_string()))
            .collect();

        for (key, value) in headers {
            request = request.header(key, value);
        }

        request
    }
}

impl Proto for HTTPProto {
    fn check(&self, password: &str) -> Result {
        // let response = send(self.request.body(()).unwrap());

        // TODO: checking
        println!("Checking: {}", password);

        // Result::MATCH
        Result::MISS
    }
}

struct Settings {
    config: config::Config,
    dict_file: String,
    sleep: u64,
    dict_type: String,
    proto: String,
    target: HashMap<String, Value>,
}

type ConfigResult<T> = result::Result<T, ConfigError>;

impl Settings {
    fn new() -> Self {
        let config_file = env::var("IMBRUT_CONFIG")
            .unwrap_or("config.yml".to_string());
        let dict_file = env::var("IMBRUT_PASSWORDS_FILE")
            .unwrap_or("passwords.txt".to_string());

        let config = config::Config::builder()
            .add_source(config::File::with_name(config_file.as_str()))
            .build()
            .unwrap();
        
        let sleep = config.get_int("sleep").unwrap() as u64;
        let dict_type = config.get_string("dict_type").unwrap();
        let proto = config.get_string("proto").unwrap();
        let target = config.get_table("target").unwrap();

        Self { 
            config,
            dict_file,
            sleep,
            dict_type,
            proto,
            target,
        }
    }

    fn get_string(&self, key: &str) -> ConfigResult<String> {
        self.config.get_string(key)
    }

    fn get_int(&self, key: &str) -> ConfigResult<i64> {
        self.config.get_int(key)
    }

    fn get_float(&self, key: &str) -> ConfigResult<f64> {
        self.config.get_float(key)
    }

    fn get_bool(&self, key: &str) -> ConfigResult<bool> {
        self.config.get_bool(key)
    }

    fn get_table(&self, key: &str) -> ConfigResult<Map<String, Value>> {
        self.config.get_table(key)
    }

    fn get_array(&self, key: &str) -> ConfigResult<Vec<Value>> {
        self.config.get_array(key)
    }
}

struct Progress {
    bar: ProgressBar,
}

impl Progress {
    fn new(workload: usize) -> Self {
        let bar = ProgressBar::new(workload as u64);
        Self { bar }
    }

    fn update(&self) {
        self.bar.inc(1);
    }

    fn finish(&self) {
        self.bar.finish();
    }
}

pub struct Application {
    settings: Settings,
    proto: Box<dyn Proto>,
    passwords: Box<dyn Iterator<Item = String>>,
    workload: usize,
}

impl Application {
    pub fn new() -> Self {
        let settings = Settings::new();
        
        let (
            passwords,
            workload
        ) =  Self::get_passwords(&settings);
        let proto = Self::get_proto(&settings);

        Self {
            settings,
            proto,
            passwords,
            workload,
        }
    }

    fn get_passwords(settings: &Settings) -> (Box<dyn Iterator<Item = String>>, usize) {
        match settings.dict_type.as_str() {
            "file" => {
                (
                    Box::new(PasswordsFile::new(&settings.dict_file)),
                    PasswordsFile::new(&settings.dict_file).count(),
                )
            }
            _ => {
                // TODO: raise error
                panic!("Unsupported dict type: {}", settings.dict_type);
            }
        }
    }

    fn get_proto(settings: &Settings) -> Box<dyn Proto> {
        match settings.proto.as_str() {
            "http" => {
                Box::new(HTTPProto::new(&settings.target))
            }
            _ => {
                // TODO: raise error
                panic!("Unsupported protocol: {}", settings.proto);
            }
        }
    }


    /// Application entrypoint.
    /// 
    /// # Examples:
    /// 
    /// Basic usage:
    /// 
    /// ```
    /// let app = Application::new();
    /// app.run();
    /// ```
    pub fn run(&self) {
        let progress = Progress::new(self.workload);
        for password in self.passwords {
            match self.proto.check(&password) {
                Result::MATCH => {
                    // TODO
                    break;
                },
                Result::MISS => {
                    // TODO
                },
            }
            if self.settings.sleep > 0 {
                thread::sleep(time::Duration::from_millis(self.settings.sleep));
            }
            progress.update();
        }
        progress.finish();
    }
}