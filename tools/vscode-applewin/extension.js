// Minimal VS Code extension: runs the applewin-dap executable as the debug adapter.
// It only starts the adapter; every request is answered by the emulator process itself.
const vscode = require("vscode");

class Factory {
  createDebugAdapterDescriptor() {
    const path = vscode.workspace
      .getConfiguration("applewin")
      .get("adapterPath", "applewin-dap");
    return new vscode.DebugAdapterExecutable(path, []);
  }
}

function activate(context) {
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterDescriptorFactory("applewin", new Factory())
  );
}

function deactivate() {}

module.exports = { activate, deactivate };
