import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import AppUsageInfo from "./components/AppUsageList";

export interface IAppUsageInfo {
  appName: string;
  totalHours: number;
  idleHours: number;
  activePercentage: number;
}

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  const [appUsageInfo, setAppUsageInfo] = useState<IAppUsageInfo[]>()
  useEffect(() => {
    const dummyData: IAppUsageInfo[] = [
      {
        appName: "Visual Studio Code",
        totalHours: 5.5,
        idleHours: 1.2,
        activePercentage: 78.18,
      },
      {
        appName: "Google Chrome",
        totalHours: 3.2,
        idleHours: 0.5,
        activePercentage: 84.38,
      },
      {
        appName: "Spotify",
        totalHours: 2.1,
        idleHours: 0.8,
        activePercentage: 61.90,
      },
    ];
    getAppUsageDetails()
    setAppUsageInfo(dummyData)
  }, [])

  async function getAppUsageDetails()
   {
    await invoke("fetch_app_usage_info").then((res) => {
      console.log(`Message: ${res}`)
    }).catch((e) => {
      console.log(e)
    })
  }
  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name }));
  }

  const iterate_over_list = appUsageInfo?.map((val: IAppUsageInfo) => {
    return <AppUsageInfo appName = {val.appName} totalHours = {val.totalHours} idleHours = {val.idleHours} activePercentage = {val.activePercentage}/>
  })

  return (
    <main className="container">
      <div>{iterate_over_list}</div>
      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault();
          greet();
        }}
      >
        <input
          id="greet-input"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter a name..."
        />
        <button type="submit">Greet</button>
      </form>
      <p>{greetMsg}</p>
    </main>
  );
}

export default App;
