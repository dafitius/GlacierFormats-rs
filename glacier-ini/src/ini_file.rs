use crate::encryption::xtea::Xtea;
use crate::encryption::xtea::XteaError;
use indexmap::IndexMap;
use itertools::Itertools;
use std::io::Write;
use std::ops::{Index, IndexMut};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IniFileError {
    #[error("Option ({}) not found", _0)]
    OptionNotFound(String),

    #[error("Can't find section ({})", _0)]
    SectionNotFound(String),

    #[error("An error occurred when parsing: {}", _0)]
    ParsingError(String),

    #[error("An io error occurred: {}", _0)]
    IoError(#[from] std::io::Error),

    #[error("An io error occurred: {}", _0)]
    DecryptionError(#[from] XteaError),

    #[error("The given input was incorrect: {}", _0)]
    InvalidInput(String),

    #[error("The requested include addition already exists: {}", _0)]
    IncludeAlreadyExists(String),
}

impl IniFileSection {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            options: IndexMap::new(),
        }
    }

    pub fn name(&self) -> String {
        self.name.to_owned()
    }

    pub fn options(&self) -> &IndexMap<String, String> {
        &self.options
    }

    pub fn has_option(&self, option_name: &str) -> bool {
        self.options.contains_key(option_name)
    }

    pub fn option(&self, option_name: &str) -> Option<String> {
        self.options.get(option_name).cloned()
    }
    
    pub fn with_option(&mut self, option_name: &str, value: &str) -> &mut Self {
        self.insert(option_name, value);
        self
    }

    pub fn insert(&mut self, option_name: &str, value: &str) {
        if let Some(key) = self.options.get_mut(option_name) {
            *key = value.to_string();
        } else {
            self.options
                .insert(option_name.to_string(), value.to_string());
        }
    }

    pub fn write_section<W: std::fmt::Write>(&self, writer: &mut W) {
        writeln!(writer, "[{}]", self.name).unwrap();
        for (key, value) in &self.options {
            writeln!(writer, "{key}={value}").unwrap();
        }
        writeln!(writer).unwrap();
    }
}


#[derive(Default, Debug, Eq, PartialEq)]
pub struct IniFileSection {
    pub(crate) name: String,
    pub(crate) options: IndexMap<String, String>,
}

/// Represents a system config file for the Glacier engine
/// ## Example contents
///
/// ```txt
/// [application]
/// ForceVSync=0
/// CapWorkerThreads=1
/// SCENE_FILE=assembly:/path/to/scene.entity
/// ....
///
/// [Hitman5]
/// usegamecontroller=1
/// ConsoleCmd UI_EnableMouseEvents 0
/// ....
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct IniFile {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) includes: Vec<IniFile>,
    pub(crate) sections: HashMap<String, IniFileSection>,
    pub(crate) console_cmds: Vec<String>,
}

impl Index<&str> for IniFileSection {
    type Output = str;

    fn index(&self, option_name: &str) -> &str {
        self.options.get(option_name).expect("Option not found")
    }
}

impl IndexMut<&str> for IniFileSection {
    fn index_mut(&mut self, option_name: &str) -> &mut str {
        self.options.entry(option_name.to_string()).or_default()
    }
}

impl Default for IniFile {
    fn default() -> Self {
        Self {
            name: "thumbs.dat".to_string(),
            description: Some(String::from("System config file for the engine")),
            includes: vec![],
            sections: Default::default(),
            console_cmds: vec![],
        }
    }
}

impl IniFile {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: None,
            includes: vec![],
            sections: Default::default(),
            console_cmds: vec![],
        }
    }
    pub fn name(&self) -> String {
        self.name.to_string()
    }
    pub fn sections(&self) -> &HashMap<String, IniFileSection> {
        &self.sections
    }

    pub fn includes(&self) -> &Vec<IniFile> {
        &self.includes
    }

    pub fn get_or_add_include(&mut self, include_name: &str) -> &mut IniFile {
        if self.find_include_mut(include_name).is_none() {
            self.add_include(IniFile::new(include_name)).unwrap();
        }
        self.find_include_mut(include_name).unwrap()
    }
    
    pub fn find_include(&self, include_name: &str) -> Option<&IniFile> {
        self.includes.iter().find(|incl| incl.name == include_name)
    }

    pub fn find_include_mut(&mut self, include_name: &str) -> Option<&mut IniFile> {
        self.includes.iter_mut().find(|incl| incl.name == include_name)
    }

    pub fn get_option(
        &self,
        section_name: &str,
        option_name: &str,
    ) -> Result<String, IniFileError> {
        match self.sections.get(section_name) {
            Some(v) => match v.options.get(option_name) {
                Some(o) => Ok(o.clone()),
                None => Err(IniFileError::OptionNotFound(option_name.to_string())),
            },
            None => Err(IniFileError::SectionNotFound(section_name.to_string())),
        }
    }

    pub fn set_description(&mut self, description: &str){
        self.description = Some(description.to_string())
    }

    pub fn with_description(&mut self, description: &str) -> &mut Self{
        self.description = Some(description.to_string());
        self
    }

    pub fn with_command(&mut self, command: &str) -> &mut Self {
        self.console_cmds.push(command.to_string());
        self
    }
    
    pub fn add_section(&mut self, section: IniFileSection) {
        self.sections.insert(section.name.to_owned(), section);
    }

    pub fn with_section(&mut self, name: &str) -> &mut IniFileSection { 
        self.add_section(IniFileSection::new(name));
        self.sections.get_mut(name).unwrap()
    }

    pub fn add_new_section(&mut self, section_name: &str, values: Option<Vec<(&str, &str)>>){
        match self.sections.get_mut(section_name){
            None => {
                self.sections.insert(section_name.to_string(), IniFileSection::new(section_name));
                self.add_new_section(section_name, values);
            }
            Some(section) => {
                if let Some(values) = values{
                    for (key, val) in values {
                        section.insert(key, val);
                    }
                }
            }
        }
    }
    
    pub fn section(&self, name: &str) -> Option<&IniFileSection> {
        self.sections.get(name)
    }

    pub fn section_mut(&mut self, name: &str) -> Option<&mut IniFileSection> {
        self.sections.get_mut(name)
    }

    pub fn set_value(
        &mut self,
        section_name: &str,
        option_name: &str,
        value: &str,
    ) -> Result<(), IniFileError> {
        match self.sections.get_mut(section_name) {
            Some(v) => match v.options.get_mut(option_name) {
                Some(o) => {
                    *o = value.to_string();
                    Ok(())
                }
                None => Err(IniFileError::OptionNotFound(option_name.to_string())),
            },
            None => Err(IniFileError::SectionNotFound(section_name.to_string())),
        }
    }

    pub fn push_console_command(&mut self, command: String) {
        self.console_cmds.push(command);
    }

    pub fn add_include(&mut self, include: IniFile) -> Result<(), IniFileError>{
        if self.includes.contains(&include){
            return Err(IniFileError::IncludeAlreadyExists(include.name))
        }
        self.includes.push(include);
        Ok(())
    }
    
    pub fn console_cmds(&self) -> &Vec<String> {
        &self.console_cmds
    }

    pub fn write_to_file<W: Write>(&self, writer: &mut W) -> Result<(), IniFileError>{
        let mut string = String::new();
        self.write_ini(&mut string);
        let data = Xtea::encrypt_text_file(string)?;
        writer.write_all(data.as_slice()).map_err(IniFileError::IoError)
    }

    pub(crate) fn write_ini<W: std::fmt::Write>(&self, writer: &mut W) {
        if let Some(description) = &self.description {
            writeln!(writer, "# {description}").unwrap();
            writeln!(writer, "\n# -----------------------------------------------------------------------------\n", ).unwrap();
        }

        for section_name in self
            .sections
            .keys()
            .sorted_by(|a, b| Ord::cmp(&a.to_lowercase(), &b.to_lowercase()))
        {
            if let Some(section) = self.sections().get(section_name) {
                section.write_section(writer);
            }
        }
        for console_cmd in &self.console_cmds {
            writeln!(writer, "ConsoleCmd {console_cmd}").unwrap();
        }
        if !self.includes.is_empty(){
            writeln!(writer).unwrap();
        }
        for include in &self.includes {
            writeln!(writer, "!include {}", include.name).unwrap();
        }
    }
}

impl Index<&str> for IniFile {
    type Output = IniFileSection;

    fn index(&self, section_name: &str) -> &IniFileSection {
        self.sections.get(section_name).expect("Section not found")
    }
}

impl IndexMut<&str> for IniFile {
    fn index_mut(&mut self, section_name: &str) -> &mut IniFileSection {
        self.sections
            .entry(section_name.to_string())
            .or_insert(IniFileSection::new(section_name))
    }
}
