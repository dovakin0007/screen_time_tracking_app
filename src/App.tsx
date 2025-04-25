import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LazyStore } from "@tauri-apps/plugin-store";

import AppUsageCard from "./components/AppUsageList";
import AppLauncherCard from "./components/AppLauncherItem";

import Box from "@mui/material/Box";
import Grid from "@mui/material/Grid";
import Typography from "@mui/material/Typography";
import CircularProgress from "@mui/material/CircularProgress";
import Paper from "@mui/material/Paper";
import Tabs from "@mui/material/Tabs";
import Tab from "@mui/material/Tab";
import List from "@mui/material/List";
import TextField from "@mui/material/TextField";
import Button from "@mui/material/Button";

import { AdapterDayjs } from "@mui/x-date-pickers/AdapterDayjs";
import { LocalizationProvider } from "@mui/x-date-pickers/LocalizationProvider";
import { DatePicker } from "@mui/x-date-pickers/DatePicker";
import dayjs, { Dayjs } from "dayjs";

export interface IAppUsageInfo {
  appName: string;
  totalHours: number;
  idleHours: number;
  activePercentage: number;
  timeLimit: number | null;
  shouldAlert: boolean | null;
  shouldClose: boolean | null;
  alertBeforeClose: boolean | null;
  alertDuration: number | null;
}

export interface ShellLinkInfo {
  link: string;
  target_path: string;
  arguments?: string;
  icon_base64_image?: string;
  working_directory?: string;
  description?: string;
}

class DatePickerDates {
  start_date: Date;
  end_date: Date;

  constructor() {
    const now = new Date();
    const sevenDaysAgo = new Date();
    sevenDaysAgo.setDate(now.getDate() - 7);
    this.start_date = sevenDaysAgo;
    this.end_date = now;
  }
}

interface TabPanelProps {
  children?: React.ReactNode;
  index: number;
  value: number;
}

function TabPanel({ children, value, index, ...other }: TabPanelProps) {
  return (
    <div
      role="tabpanel"
      hidden={value !== index}
      id={`tabpanel-${index}`}
      aria-labelledby={`tab-${index}`}
      {...other}
    >
      {value === index && <Box sx={{ p: 3 }}>{children}</Box>}
    </div>
  );
}

function App() {
  const launcher_store = new LazyStore("app_launcher_store.json");
  const defaultDates = new DatePickerDates();
  const [appUsageInfo, setAppUsageInfo] = useState<IAppUsageInfo[] | null>(
    null,
  );
  const [loading, setLoading] = useState<boolean>(false);
  const [startDate, setStartDate] = useState<Dayjs>(
    dayjs(defaultDates.start_date),
  );
  const [endDate, setEndDate] = useState<Dayjs>(dayjs(defaultDates.end_date));
  const [fetchError, setFetchError] = useState<string | null>(null);
  const [tabIndex, setTabIndex] = useState<number>(0);
  const [search, setSearch] = useState("");
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [shellLinks, setShellLinks] = useState<ShellLinkInfo[]>([]);
  const [launchDataMap, setLaunchDataMap] = useState<
    Record<string, { count: number; lastLaunched: string | null }>
  >({});
  const [loadingShellLinks, setLoadingShellLinks] = useState(true);
  const [sortAscending, setSortAscending] = useState(true);

  useEffect(() => {
    async function loadLinks() {
      try {
        const links: ShellLinkInfo[] = await invoke("fetch_shell_links");
        setShellLinks(links);
        const map: Record<
          string,
          { count: number; lastLaunched: string | null }
        > = {};
        for (const link of links) {
          const data = await launcher_store.get<{
            count: number;
            lastLaunched: string;
          }>(link.link);
          map[link.link] = {
            count: data?.count || 0,
            lastLaunched: data?.lastLaunched || null,
          };
        }
        setLaunchDataMap(map);
      } catch (e) {
        console.error("Failed to load shell links:", e);
      } finally {
        setLoadingShellLinks(false);
      }
    }

    loadLinks();
  }, []);

  useEffect(() => {
    getAppUsageDetails();
  }, [startDate, endDate]);

  const handleTabChange = (_event: React.SyntheticEvent, newValue: number) => {
    setTabIndex(newValue);
  };

  async function getAppUsageDetails() {
    setLoading(true);
    setFetchError(null);
    setAppUsageInfo(null);

    try {
      const formattedStartDate = startDate.format("YYYY-MM-DD");
      const formattedEndDate = endDate.format("YYYY-MM-DD");

      const res = await invoke("fetch_app_usage_info", {
        startDate: formattedStartDate,
        endDate: formattedEndDate,
      });

      if (Array.isArray(res)) {
        const mappedData: IAppUsageInfo[] = res.map((item: any) => ({
          appName: item.appName || item.app_name,
          totalHours: item.totalHours || item.total_hours || 0,
          idleHours: item.idleHours || item.idle_hours || 0,
          activePercentage: item.activePercentage ?? item.active_percentage ??
            0,
          timeLimit: item.timeLimit ?? item.time_limit ?? null,
          shouldAlert: item.shouldAlert ?? item.should_alert ?? null,
          shouldClose: item.shouldClose ?? item.should_close ?? null,
          alertBeforeClose: item.alertBeforeClose ?? item.alert_before_close ??
            null,
          alertDuration: item.alertDuration ?? item.alert_duration ?? null,
        }));
        setAppUsageInfo(mappedData);
      } else {
        setAppUsageInfo([]);
      }
    } catch (e: any) {
      console.error("Error fetching app usage info:", e);
      setFetchError(`Failed to load data: ${e.message || "Unknown error"}`);
      setAppUsageInfo([]);
    } finally {
      setLoading(false);
    }
  }

  const handleStartDateChange = (newValue: Dayjs | null) => {
    if (newValue && newValue.isValid()) {
      setStartDate(newValue);
    }
  };

  const handleEndDateChange = (newValue: Dayjs | null) => {
    if (newValue && newValue.isValid()) {
      setEndDate(newValue);
    }
  };

  const filteredApps = shellLinks
    .map((link) => {
      const appName = link.link.split("\\").pop()?.replace(/\.lnk$/i, "") ||
        "App";
      return {
        ...link,
        displayName: appName.toLowerCase(),
      };
    })
    .filter((link) => link.displayName.includes(search.toLowerCase()))
    .sort((a, b) => {
      return sortAscending
        ? a.displayName.localeCompare(b.displayName)
        : b.displayName.localeCompare(a.displayName);
    });

  return (
    <LocalizationProvider dateAdapter={AdapterDayjs}>
      <Box
        sx={{
          py: 4,
          px: { xs: 1, sm: 2, md: 3 },
          maxWidth: "lg",
          mx: "auto",
          background: "linear-gradient(to right, #e3f2fd, #fce4ec)",
          borderRadius: 4,
          boxShadow: 4,
          minHeight: "100vh",
        }}
      >
        <Typography
          variant="h4"
          component="h1"
          gutterBottom
          align="center"
          color="primary"
        >
          App Dashboard
        </Typography>

        <Box sx={{ borderBottom: 1, borderColor: "divider" }}>
          <Tabs value={tabIndex} onChange={handleTabChange} centered>
            <Tab label="App Usage" />
            <Tab label="App Launcher" />
          </Tabs>
        </Box>

        <TabPanel value={tabIndex} index={0}>
          <Box
            sx={{
              mb: 4,
              p: 3,
              borderRadius: "12px",
              background: "#ffffff",
              boxShadow: 3,
            }}
          >
            <Typography variant="h6" gutterBottom>Select Date Range</Typography>
            <Grid container spacing={3}>
              <Grid size={{ xs: 12, sm: 6 }}>
                <DatePicker
                  label="Start Date"
                  value={startDate}
                  onChange={handleStartDateChange}
                  format="YYYY-MM-DD"
                  sx={{ width: "100%" }}
                />
              </Grid>
              <Grid size={{ xs: 12, sm: 6 }}>
                <DatePicker
                  label="End Date"
                  value={endDate}
                  onChange={handleEndDateChange}
                  format="YYYY-MM-DD"
                  sx={{ width: "100%" }}
                />
              </Grid>
            </Grid>
          </Box>

          <Typography variant="h5" sx={{ mt: 4 }} color="secondary">
            Usage Details
          </Typography>

          {loading && (
            <Box sx={{ display: "flex", justifyContent: "center", my: 4 }}>
              <CircularProgress />
              <Typography sx={{ ml: 2 }}>Loading usage data...</Typography>
            </Box>
          )}

          {fetchError && (
            <Box sx={{ display: "flex", justifyContent: "center", my: 4 }}>
              <Typography color="error">{fetchError}</Typography>
            </Box>
          )}

          {!loading && !fetchError &&
            (!appUsageInfo || appUsageInfo.length === 0) && (
            <Box sx={{ display: "flex", justifyContent: "center", my: 4 }}>
              <Typography>
                No app usage data found for the selected date range.
              </Typography>
            </Box>
          )}

          {appUsageInfo && appUsageInfo.length > 0 && (
            <Grid container spacing={3} direction="column">
              {appUsageInfo.map((val, idx) => (
                <Grid key={idx}>
                  <Paper
                    elevation={3}
                    sx={{
                      p: 2,
                      borderRadius: 4,
                      transition: "all 0.3s",
                      "&:hover": {
                        transform: "scale(1.02)",
                        boxShadow: 6,
                        backgroundColor: "#f3f4f6",
                      },
                    }}
                  >
                    <AppUsageCard {...val} />
                  </Paper>
                </Grid>
              ))}
            </Grid>
          )}
        </TabPanel>

        <TabPanel value={tabIndex} index={1}>
          <Typography variant="h6" gutterBottom>App Launcher</Typography>
          <TextField
            fullWidth
            label="Search apps"
            variant="outlined"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            sx={{ my: 2 }}
          />
          <Box sx={{ display: "flex", justifyContent: "flex-end", mb: 2 }}>
            <Button
              variant="contained"
              onClick={() => setSortAscending((prev) => !prev)}
            >
              Sort: {sortAscending ? "Ascending" : "Descending"}
            </Button>
          </Box>

          {loadingShellLinks
            ? (
              <Box sx={{ display: "flex", justifyContent: "center", my: 4 }}>
                <CircularProgress />
              </Box>
            )
            : (
              <Paper variant="outlined">
                <List>
                  {filteredApps.map((app, idx) => {
                    const launchInfo = launchDataMap[app.link] ||
                      { count: 0, lastLaunched: null };

                    return (
                      <AppLauncherCard
                        key={idx}
                        app={app}
                        expanded={expandedId === idx}
                        onToggle={() =>
                          setExpandedId(expandedId === idx ? null : idx)}
                        launchCount={launchInfo.count}
                        lastLaunched={launchInfo.lastLaunched}
                        onLaunch={async () => {
                          try {
                            await invoke("start_app", { link: app.link });
                            const current = await launcher_store.get<
                              { count: number; lastLaunched: string }
                            >(app.link);
                            const newData = {
                              count: (current?.count || 0) + 1,
                              lastLaunched: new Date().toISOString(),
                            };
                            await launcher_store.set(app.link, newData);
                            await launcher_store.save();
                            setLaunchDataMap((prev) => ({
                              ...prev,
                              [app.link]: newData,
                            }));
                          } catch (error) {
                            console.error("Failed to launch app:", error);
                          }
                        }}
                      />
                    );
                  })}
                </List>
              </Paper>
            )}
        </TabPanel>
      </Box>
    </LocalizationProvider>
  );
}

export default App;
