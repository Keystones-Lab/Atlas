import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  // const [hello, setHello] = useState("");
  const [systemInfo, setSystemInfo] = useState(null)
  // const [cpu, setCpu] = useState(0);
  // const [memory, setMemory] = useState(0)

  // async function helloAtlas() {
  //   setHello(await invoke("hello_atlas"));
  // }

  async function getSystemInfo() {
    setSystemInfo(await invoke("get_system_info"))
  }

  return (
    <main className="container">
      {/* <button onClick={helloAtlas}>Click me</button>
      <p>{hello}</p> */}
      <button onClick={getSystemInfo}>Get Diagnostics</button>
      <p>CPU: {systemInfo.cpu_percent}%</p>
      <p>RAM: {systemInfo.used_memory} / {systemInfo.total_memory}</p>
      <p>DISK: {systemInfo.used_disk} / {systemInfo.total_disk}</p>
    </main>
  );
}

export default App;
