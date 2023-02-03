mod proto {
    use std::any::Any;
    use std::collections::HashMap;
    use crate::application::Application;

    use itertools::Itertools;
    use reqwest::{
        self, 
        header::{HeaderMap, HeaderName, HeaderValue}, 
        blocking::RequestBuilder
    };

    type CheckResult = Result<(), ()>;

    trait Credentials {}

    pub trait Proto {
        type Creds;
    
        fn check(&self, creds: &Self::Creds) -> CheckResult;
        fn get_credentials(&self) -> Box<dyn Iterator<Item = Self::Creds>>;

        fn get_workload(&self) -> usize {
            self.get_credentials().count()
        }
    }

    pub struct DynProto<P, C> 
        where 
            P: Proto<Creds = C>, 
            C: Credentials + 'static 
    {
        proto: P
    }
    
    impl<P, C> Proto for DynProto<P, C> 
        where 
            P: Proto<Creds = C>, 
            C: Credentials + 'static 
    {
        type Creds = Box<dyn Any>;
    
        fn check(&self, creds: &Self::Creds) -> CheckResult {
            if let Some(creds) = creds.downcast_ref::<C>() {
                self.proto.check(creds)
            } else {
                panic!("Credentials are not valid")
            }
        }

        fn get_credentials(&self) -> Box<dyn Iterator<Item = Self::Creds>> {
            Box::new(self.proto.get_credentials())
        }
    }

    pub struct HTTPProto<'a> {
        app: &'a Application,
        auth_type: String,
        success_codes: Vec<http::StatusCode>,
        request: RequestBuilder,
        success_if_contains: Vec<String>,
        fail_if_contains: Vec<String>,
    }

    impl HTTPProto<'_> {
        pub fn new(app: &Application, target: &HashMap<String, config::Value>) -> Self {
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
    
            let success_if_contains: Vec<String> = target.get("success_if_containes").unwrap().clone()
                .into_array()
                .unwrap()
                .into_iter()
                .map(|x| x.to_string())
                .collect(); // TODO
            
            let fail_if_contains: Vec<String> = target.get("fail_if_containes").unwrap().clone()
                .into_array()
                .unwrap()
                .into_iter()
                .map(|x| x.to_string())
                .collect(); // TODO
            
            let request = Self::build_request(&target);
    
            Self { 
                app,
                auth_type,
                success_codes,
                request,
                success_if_contains,
                fail_if_contains,
            }
        }
    
        fn build_request(target: &HashMap<String, config::Value>) -> RequestBuilder {
            let uri = target.get("uri").unwrap().to_string();
    
            let method = target.get("method").unwrap().to_string(); // TODO: default POST
            let method = http::Method::from_bytes(method.as_bytes()).unwrap();
    
            let client = reqwest::blocking::Client::new();  // TODO: add retry strategy
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
    
            request.headers(headers)
        }
    }

    struct HTTPCredentials {
        // TODO: add form field names info
        username: String,
        password: String,
    }

    // impl HTTPCredentials {
    //     fn into_pairs(&self) -> [(String, String); 2] {
    //         [
    //             ("username".to_string(), self.username), 
    //             ("password".to_string(), self.password),
    //         ]
    //     }
    // }
    
    impl Credentials for HTTPCredentials {}

    impl Proto for HTTPProto<'_> {
        type Creds = HTTPCredentials;
    
        fn check(&self, creds: &Self::Creds) -> CheckResult {
            let mut request = self.request.try_clone().unwrap();
    
            let username = &creds.username;
            let password = &creds.password;
    
            match self.auth_type.as_str() {
                "form" => {
                    // TODO: custom form field names
                    request = request.form(&[("username", username), ("password", password)]);
                }
                "basic" => {
                    request = request.basic_auth(username, Some(password));
                }
                _ => {
                    panic!("Unsupported authentication type: {}", self.auth_type)
                }
            }
            
            let response = request.send().unwrap();
    
            let response_status = response.status();
            let response_content = response.text().unwrap();
    
            if self.success_codes.contains(&response_status) {
                for x in &self.fail_if_contains {
                    if response_content.contains(x) {
                        return Err(());
                    }
                }
                for x in &self.success_if_contains {
                    if response_content.contains(x) {
                        return Ok(());
                    }
                }
            }
    
            Err(())
        }
    
        fn get_credentials(&self) -> Box<dyn Iterator<Item = Self::Creds>> {
            let usernames = self.app.get_usernames();
            let passwords = self.app.get_passwords();

            let r = Box::new("αβ".chars().clone()).clone();
            (0..2).cartesian_product(r);

            Box::new(
                usernames
                    .cartesian_product(passwords)
                    .map(|(username, password)| Self::Creds {username, password})
            )

            // todo!()
        }
    }    

    #[cfg(test)]
    mod test {
        // TODO
    }
}

mod utils {
    use std::fs::File;
    use std::io::{BufReader, BufRead, Lines};
    use std::str::Chars;

    use itertools::{Itertools, CombinationsWithReplacement};

    // #[derive(Clone)]
    pub struct FileWithStrings {
        iter: Lines<BufReader<File>>,
    }
    
    impl FileWithStrings {
        pub fn new(path: &str) -> Self {
            let file = File::open(path).unwrap();
            let reader = BufReader::new(file);
            Self { iter: reader.lines() }
        }
    }
    
    impl Iterator for FileWithStrings {
        type Item = String;
    
        fn next(&mut self) -> Option<Self::Item> {
            self.iter.next().and_then(|r| r.ok())
        }
    }

    // #[derive(Clone)]
    pub struct StringsGenerator<'a> {
        iter: CombinationsWithReplacement<Chars<'a>>,
    }
    
    impl StringsGenerator<'_> {
        // FIXME: combinations_with_replacement is not what we want here.
        pub fn new(allowed_chars: &Vec<String>, size: usize) -> Self {
            let iter = allowed_chars
                .concat()
                .chars()
                .combinations_with_replacement(size);
            Self { iter }
        }
    }
    
    impl Iterator for StringsGenerator<'_> {
        type Item = String;
    
        fn next(&mut self) -> Option<Self::Item> {
            self.iter.next().and_then(|r| Some(r.into_iter().collect()))
        }
    }

    #[cfg(test)]
    mod test {
        use super::{StringsGenerator, FileWithStrings};

        #[test]
        fn test_file_with_strings() {
            let path = "strings.txt";
            let strings: Vec<String> = FileWithStrings::new(path).collect();
            assert_eq!(strings, vec!["test1", "test2", "test3"]);
        }

        #[test]
        fn test_strings_generator() {
            let allowed_chars = vec![String::from("123")];
            let strings: Vec<String> = StringsGenerator::new(&allowed_chars, 3).collect();
            assert_eq!(strings, vec![
                "111", "222", "333",
                "122", "212", "221", "211", "121", "112",
                "233", "323", "332", "322", "232", "223",
                "133", "313", "331", "311", "131", "113",
                "123", "132", "213", "231", "321", "312",
            ]);
        }
    }
}

mod settings {
    use std::env;
    use std::collections::HashMap;

    pub struct Settings {
        pub usernames_file: String,
        pub passwords_file: String,
        pub dict_type: String,
        pub proto: String,
        pub target: HashMap<String, config::Value>,
        pub password_len: usize,
        pub allowed_chars: Vec<String>,
        pub strategy: Vec<(String, u64)>,
    }
    
    impl Settings {
        pub fn new() -> Self {
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

            let strategy: Vec<(String, u64)> = config.get_array("strategy").unwrap().iter()
                .map(|x| x.into_table().unwrap())
                .map(|x| {
                    x.into_iter().map(|(k, v)| (k, v.into_uint().unwrap())).next()
                })
                .map(|x| x.unwrap())
                .collect(); // TODO: empty by default
    
            Self { 
                usernames_file,
                passwords_file,
                dict_type,
                proto,
                target,
                password_len,
                allowed_chars,
                strategy,
            }
        }
    
        fn save() {
            // TODO: save data into yaml file
        }
    }

    #[cfg(test)]
    mod test {
        // TODO: unit tests
    }
}

mod ui {
    use indicatif::{ProgressBar, ProgressStyle};

    pub trait UIApplication {
        fn run(&self);
        // fn update(&self);
        // fn complete(&self);
    }

    pub struct UI<'a> {
        version: &'a str,
        progress: Progress,
    }

    impl UI<'_> {
        pub fn new(version: &str, workload: usize) -> Self {
            let progress = Progress::new(workload);

            Self { 
                version,
                progress,
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
    }

    impl UIApplication for UI<'_> {
        fn run(&self) {
            self.show_splash();
        }
    }

    pub struct Progress { 
        pb: ProgressBar,
    }
    
    impl Progress {
        pub fn new(workload: usize) -> Self {
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
    
        pub fn update(&mut self, item: String) {
            let msg = format!("current: {}", item);
            self.pb.set_message(msg);
            self.pb.inc(1);
        }
    
        pub fn complete(&self, item: Option<String>) {
            if let Some(item) = item {
                let msg = format!("match: {}", item);
                self.pb.abandon_with_message(msg);
            } else {
                self.pb.abandon();
            }
        }
    }

    #[cfg(test)]
    mod test {
        // TODO: unit tests
    }
}

mod strategy {
    use std::any::Any;
    use std::{thread, time};

    use crate::proto::Proto;
    use crate::ui::UIApplication;

    pub struct Strategy {
        proto: Box<dyn Proto<Creds = Box<dyn Any>>>,
        states: Option<Vec<Box<dyn State>>>,
        credentials: Box<dyn Iterator<Item = (usize, Box<dyn Any>)>>,
        ui: Option<Box<dyn UIApplication>>,
    }

    impl Strategy {
        pub fn new<'a>(proto: Box<dyn Proto<Creds = Box<dyn Any>>>) -> Self {
            Self {
                proto,
                credentials: Box::new(proto.get_credentials().enumerate()),
                states: None,
                ui: None,
            }
        }
    }

    trait State {
        fn run(&self) -> Option<()>;
    }
    struct SleepState<'a> {value: u64, strategy: &'a Strategy}
    struct RequestsState<'a> {value: u64, strategy: &'a Strategy}
    struct DefaultState<'a> {strategy: &'a Strategy}

    impl State for SleepState<'_> {
        fn run(&self) -> Option<()> {
            thread::sleep(time::Duration::from_millis(self.value));
            None
        }
    }

    impl State for RequestsState<'_> {
        fn run(&self) -> Option<()> {
            for (i, creds) in self.strategy.credentials {
                if let Some(ui) = self.strategy.ui {
                    // TODO: send message to UI for updating progress
                }
                if let Ok(_) =  self.strategy.proto.check(&creds) {
                    if let Some(ui) = self.strategy.ui {
                        // TODO: send message to UI. Processing finished
                    }
                    return Some(());
                } else {
                    if (i as u64) % self.value  == self.value - 1 {
                        return None;
                    }
                }
            }
            Some(())
        }
    }

    impl State for DefaultState<'_> {
        fn run(&self) -> Option<()> {
            for (_, creds) in self.strategy.credentials {
                if let Some(ui) = self.strategy.ui {
                    // TODO: send message to UI for updating progress
                }
                if let Ok(_) =  self.strategy.proto.check(&creds) {
                    if let Some(ui) = self.strategy.ui {
                        // TODO: send message to UI. Processing finished
                    }
                    return Some(());
                }
            }
            Some(())
        }
    }

    impl Strategy {
        pub fn run(&self) {
            for state in self.states.unwrap().iter().cycle() {
                if let Some(_) = state.run() {
                    break;
                }
            }
        }

        pub fn set_ui(&self, ui: Box<dyn UIApplication>) -> &Self {
            self.ui = Some(ui);
            self
        }

        pub fn set_strategy(&self, raw_strategy: &Vec<(String, u64)>) -> &Self {
            let states: Vec<Box<dyn State>> = vec![Box::new(DefaultState{strategy: self})];
            if !raw_strategy.is_empty() {
                let states: Vec<Box<dyn State>> = raw_strategy.iter()
                    .map(|(key, value)| {
                        match key.as_str() {
                            "requests" => {
                                Box::new(RequestsState{value: *value, strategy: self}) as Box<dyn State>
                            },
                            "sleep" => {
                                Box::new(SleepState{value: *value, strategy: self}) as Box<dyn State>
                            },
                            _ => {
                                panic!("Unsupported strategy key: {}", key)
                            }
                        }
                    })
                    .collect();
            }
            self.states = Some(states);
            self
        }
    }

    #[cfg(test)]
    mod test {
        // TODO: unit tests
    }
}

mod application {
    use std::any::Any;
    use std::env;

    use crate::proto::{HTTPProto, DynProto, Proto};
    use crate::settings::Settings;
    use crate::utils::{FileWithStrings, StringsGenerator};
    use crate::strategy::Strategy;
    use crate::ui::{UI, UIApplication};
    
    pub struct Application {
        settings: Settings,
        version: String,
    }
    
    impl Application {
        pub fn new() -> Self {
            let settings = Settings::new();
            let version = env!("CARGO_PKG_VERSION").to_string();
    
            Self {
                settings,
                version,
            }
        }
    
        /// Get protocol according to settings
        fn get_proto(&self) -> Box<dyn Proto<Creds = Box<dyn Any>>> {
            match self.settings.proto.as_str() {
                "http" => {
                    let proto = HTTPProto::new(&self, &self.settings.target);
                    Box::new(DynProto { proto })
                }
                _ => {
                    panic!("Unsupported protocol: {}", self.settings.proto);
                }
            }
        }
    
        /// Passwords stream
        pub fn get_passwords(&self) -> Box<dyn Iterator<Item = String>> {
            match self.settings.dict_type.as_str() {
                "file" => {
                    let passwords_file = &self.settings.passwords_file;
                    Box::new(FileWithStrings::new(passwords_file))
                }
                "generator" => {
                    let allowed_chars = &self.settings.allowed_chars;
                    let password_len = self.settings.password_len;
                    Box::new(StringsGenerator::new(allowed_chars, password_len))
                }
                _ => {
                    panic!("Unsupported password source type: {}", self.settings.dict_type);
                }
            }
        }
    
        /// Usernames stream
        pub fn get_usernames(&self) -> Box<dyn Iterator<Item = String>> {
            todo!()
        }
    
        /// Application entrypoint
        pub fn run(&self) {
            let proto = self.get_proto();
            let ui = Box::new(UI::new(&self.version, proto.get_workload()));

            let strategy = Strategy::new(proto)
                .set_strategy(&self.settings.strategy)
                .set_ui(ui);

            ui.run();
            strategy.run();
        }
    }

    #[cfg(test)]
    mod test {
        // TODO: unit tests
    }
}