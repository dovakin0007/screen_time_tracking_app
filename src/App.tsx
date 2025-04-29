import { useState } from "react";
import { Box, Tabs, Tab } from "@mui/material";
import { LocalizationProvider } from "@mui/x-date-pickers/LocalizationProvider";
import { AdapterDayjs } from "@mui/x-date-pickers/AdapterDayjs";
import TabPanel from "./components/TabPanel";
import AppUsageTab from "./pages/AppUsageTab";
import AppLauncherTab from "./pages/AppLauncherTab";

export default function App() {
  const [tabIndex, setTabIndex] = useState(0);

  return (
    <LocalizationProvider dateAdapter={AdapterDayjs}>
      <Box /* dashboard layout here */>
        <Tabs value={tabIndex} onChange={(_, val) => setTabIndex(val)} centered>
          <Tab label="App Usage" />
          <Tab label="App Launcher" />
        </Tabs>

        <TabPanel value={tabIndex} index={0}>
          <AppUsageTab />
        </TabPanel>
        <TabPanel value={tabIndex} index={1}>
          <AppLauncherTab />
        </TabPanel>
      </Box>
    </LocalizationProvider>
  );
}
