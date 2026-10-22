use std::io;

use inkwell::{
    OptimizationLevel,
    module::Module,
    targets::{CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine},
};

use crate::{
    cli::{Cli, Emit},
    error::Error,
};

pub fn emit(cli: &Cli, module: &Module) -> Result<(), Error> {
    match cli.emit() {
        Emit::Ir => module
            .print_to_file(cli.out_path())
            .map_err(|error| Error::IO(io::Error::other(error.to_string()))),
        Emit::Obj => {
            let triple = TargetMachine::get_default_triple();
            let cpu = TargetMachine::get_host_cpu_name();
            let features = TargetMachine::get_host_cpu_features();
            Target::initialize_native(&InitializationConfig::default()).map_err(|message| {
                Error::Target {
                    triple: triple.to_string(),
                    message,
                }
            })?;
            let target = Target::from_triple(&triple).map_err(|message| Error::Target {
                triple: triple.as_str().to_string_lossy().into_owned(),
                message: message.to_string(),
            })?;
            let target_machine = target
                .create_target_machine(
                    &triple,
                    cpu.to_str().unwrap(),
                    features.to_str().unwrap(),
                    OptimizationLevel::Default,
                    RelocMode::PIC,
                    CodeModel::Default,
                )
                .ok_or_else(|| Error::Target {
                    triple: triple.as_str().to_string_lossy().into_owned(),
                    message: "cannot create a target machine".to_string(),
                })?;
            module.set_triple(&triple);
            module.set_data_layout(&target_machine.get_target_data().get_data_layout());
            target_machine
                .write_to_file(module, FileType::Object, cli.out_path())
                .map_err(|error| Error::IO(io::Error::other(error.to_string())))?;
            Ok(())
        }
    }
}
