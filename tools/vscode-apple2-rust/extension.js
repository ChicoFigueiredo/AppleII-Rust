// Minimal VS Code extension: runs the apple2-rust-dap executable as the debug adapter.
// It only starts the adapter; every request is answered by the emulator process itself.
const vscode = require("vscode");

class Factory {
  createDebugAdapterDescriptor() {
    const path = vscode.workspace
      .getConfiguration("apple2-rust")
      .get("adapterPath", "apple2-rust-dap");
    return new vscode.DebugAdapterExecutable(path, []);
  }
}

function activate(context) {
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterDescriptorFactory("apple2-rust", new Factory())
  );
}

function deactivate() {}

module.exports = { activate, deactivate };
