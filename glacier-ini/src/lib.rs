use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::from_utf8;
use pathdiff::diff_paths;
use crate::encryption::xtea::Xtea;
use crate::ini_file::{IniFile, IniFileError, IniFileSection};
use crate::utils::normalize_path;

pub mod ini_file;
mod encryption;
mod utils;


pub struct IniKey {
    pub(crate) section: String,
    pub(crate) option: String,
}

impl IniKey {
    pub fn from_tuple(section: &str, option: &str) -> Self {
        Self{
            section: section.to_string(),
            option: option.to_string(),
        }
    }
    pub fn from_path(path: &str) -> Self {
        match path.split_once("/"){
            Some((section, option)) => Self{section: section.to_string(), option: option.to_string() },
            None => Self{ section: "".to_string(), option: path.to_string() },
        }
    }
}

impl From<&str> for IniKey {
    fn from(path: &str) -> Self {
        IniKey::from_path(path)
    }
}

impl From<(&str, &str)> for IniKey {
    fn from(tuple: (&str, &str)) -> Self {
        IniKey::from_tuple(tuple.0, tuple.1)
    }
}

// impl From<(&str, &str)> for IniKey {    }

/// A hierarchical file system of [IniFile].
///
/// example usage:
/// ```ignore
///  use std::path::PathBuf;
///  use rpkg_rs::misc::ini_file_system::IniFileSystem;
///
///  let retail_path = PathBuf::from("Path to retail folder");
///  let thumbs_path = retail_path.join("thumbs.dat");
///
///  let thumbs = IniFileSystem::from(&thumbs_path.as_path())?;
///
///  let app_options = &thumbs.root()?;
///
///  if let (Some(proj_path), Some(runtime_path)) = (app_options.get("PROJECT_PATH"), app_options.get("RUNTIME_PATH")) {
///     println!("Project path: {}", proj_path);
///     println!("Runtime path: {}", runtime_path);
///  }
/// ```
#[derive(Debug)]
pub struct IniFileSystem {
    root: IniFile,
}

impl IniFileSystem {
    pub fn new(ini_file: IniFile) -> Self {
        Self {
            root: ini_file,
        }
    }

    pub fn from_path(root_file: impl AsRef<Path>) -> Result<Self, IniFileError> {
        let ini_file = Self::load_from_path(
            root_file.as_ref(),
            PathBuf::from(root_file.as_ref()).parent().unwrap(),
        )?;
        Ok(Self{
            root: ini_file
        })
    }

    fn load_from_path(path: &Path, working_directory: &Path) -> Result<IniFile, IniFileError> {
        let content = fs::read(path).map_err(IniFileError::IoError)?;
        let mut content_decrypted = from_utf8(content.as_ref()).unwrap_or("").to_string();
        if Xtea::is_encrypted_text_file(&content) {
            content_decrypted =
                Xtea::decrypt_text_file(&content).map_err(IniFileError::DecryptionError)?;
        }

        let ini_file_name = match diff_paths(path, working_directory) {
            Some(relative_path) => relative_path.to_str().unwrap().to_string(),
            None => path.to_str().unwrap().to_string(),
        };
        Self::load_from_string(
            ini_file_name.as_str(),
            content_decrypted.as_str(),
            working_directory,
        )
    }

    fn load_from_string(
        name: &str,
        ini_file_content: &str,
        working_directory: &Path,
    ) -> Result<IniFile, IniFileError> {
        let mut active_section: String = "None".to_string();
        let mut ini_file = IniFile::new(name);

        for line in ini_file_content.lines() {
            if let Some(description) = line.strip_prefix('#') {
                if ini_file_content.starts_with(line) {
                    //I don't really like this, but IOI seems to consistently use the first comment as a description.
                    ini_file.description = Some(description.trim_start().to_string());
                }
            } else if let Some(line) = line.strip_prefix('!') {
                if let Some((command, value)) = line.split_once(' ') {
                    if command == "include" {
                        let include = Self::load_from_path(
                            working_directory.join(value).as_path(),
                            working_directory,
                        )?;
                        ini_file.includes.push(include);
                    }
                }
            } else if let Some(mut section_name) = line.strip_prefix('[') {
                section_name = section_name
                    .strip_suffix(']')
                    .ok_or(IniFileError::ParsingError(
                        "a section should always have a closing ] bracket".to_string(),
                    ))?;
                active_section = section_name.to_string();
                if !ini_file.sections.contains_key(&active_section) {
                    ini_file.sections.insert(
                        active_section.clone(),
                        IniFileSection::new(&active_section.clone()),
                    );
                }
            } else if let Some(keyval) = line.strip_prefix("ConsoleCmd ") {
                ini_file.console_cmds.push(keyval.to_string());
            } else if let Some((key, val)) = line.split_once('=') {
                if let Some(section) = ini_file.sections.get_mut(&active_section) {
                    section.insert(key, val);
                }
            }
        }
        Ok(ini_file)
    }

    pub fn write_to_folder<P: AsRef<Path>>(&self, path: P) -> Result<(), IniFileError> {
        let mut folder = path.as_ref();
        if folder.is_file() {
            folder = path.as_ref().parent().ok_or(IniFileError::InvalidInput(
                "The export path cannot be empty".to_string(),
            ))?;
        }
        fn write_children_to_folder(path: &Path, ini_file: &IniFile) -> Result<(), IniFileError> {
            let mut file_path = path.join(&ini_file.name);
            file_path = normalize_path(&file_path);

            let parent_dir = file_path.parent().ok_or(IniFileError::InvalidInput(
                "Invalid export path given".to_string(),
            ))?;
            fs::create_dir_all(parent_dir)?;

            let mut writer = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&file_path)?;
            ini_file.write_to_file(&mut writer)?;

            for include in ini_file.includes.iter() {
                match write_children_to_folder(parent_dir, include) {
                    Ok(_) => {}
                    Err(e) => return Err(e),
                };
            }
            Ok(())
        }

        write_children_to_folder(folder, &self.root)
    }

    /// Normalizes the IniFileSystem by merging sections and console commands from included files into the root file.
    pub fn normalize(&mut self) {
        let mut queue: VecDeque<IniFile> = VecDeque::new();
        for include in self.root.includes.drain(0..) {
            queue.push_back(include);
        }

        while let Some(mut current_file) = queue.pop_front() {
            let root_sections = &mut self.root.sections;

            for (section_key, section) in current_file.sections.drain() {
                if !root_sections.contains_key(&section_key) {
                    root_sections.insert(section_key.clone(), section);
                } else {
                    let root_section = root_sections.get_mut(&section_key).unwrap();
                    for (key, value) in section.options {
                        if !root_section.has_option(&key) {
                            root_section.insert(&key, &value);
                        } else {
                            root_section.insert(&key, value.as_str());
                        }
                    }
                }
            }

            for console_cmd in current_file.console_cmds.drain(..) {
                if !self.root.console_cmds.contains(&console_cmd) {
                    self.root.console_cmds.push(console_cmd);
                }
            }
            for include in current_file.includes.drain(0..) {
                queue.push_back(include);
            }
        }
    }

    /// Retrieves all console commands from the IniFileSystem, including those from included files.
    pub fn console_cmds(&self) -> Vec<String> {
        let mut cmds: Vec<String> = vec![];

        // Helper function to traverse the includes recursively
        fn traverse_includes(ini_file: &IniFile, cmds: &mut Vec<String>) {
            for include in &ini_file.includes {
                cmds.extend_from_slice(&include.console_cmds);
                traverse_includes(include, cmds);
            }
        }

        cmds.extend_from_slice(&self.root.console_cmds);
        traverse_includes(&self.root, &mut cmds);

        cmds
    }

    /// Retrieves the value of an option in a section from the IniFileSystem, including values from included files.
    pub fn option(&self, key: impl Into<IniKey> + Clone) -> Result<String, IniFileError> {
        let mut queue: VecDeque<&IniFile> = VecDeque::new();
        queue.push_back(&self.root);
        let mut latest_value: Option<String> = None;

        while let Some(current_file) = queue.pop_front() {
            if let Ok(value) = current_file.get_option(&key.clone().into().section, &key.clone().into().option) {
                // Update the latest value found
                latest_value = Some(value.clone());
            }
            for include in &current_file.includes {
                queue.push_back(include);
            }
        }

        // Return the latest value found or an error if none
        latest_value.ok_or_else(|| IniFileError::OptionNotFound(key.clone().into().option.to_string()))
    }

    /// Retrieves a reference to the root IniFile of the IniFileSystem.
    pub fn root(&self) -> &IniFile {
        &self.root
    }

    /// Retrieves a reference to the root IniFile of the IniFileSystem.
    pub fn root_mut(&mut self) -> &mut IniFile {
        &mut self.root
    }
}
