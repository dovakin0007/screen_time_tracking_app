import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LazyStore } from "@tauri-apps/plugin-store";

import {
  Box,
  Button,
  CircularProgress,
  List,
  Paper,
  TextField,
  Typography,
} from "@mui/material";
import { AppLauncherCard } from "../components/AppLauncher/AppLauncherCard";

export interface ShellLinkInfo {
  link: string;
  target_path: string;
  arguments?: string;
  icon_base64_image?: string;
  working_directory?: string;
  description?: string;
}

export default function AppLauncherTab() {
  const launcher_store = new LazyStore("app_launcher_store.json");
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
    <Box
      sx={{ py: 4, px: { xs: 1, sm: 2, md: 3 }, maxWidth: "lg", mx: "auto" }}
    >
      <Typography variant="h4" align="center" gutterBottom>
        App Launcher
      </Typography>

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
          onClick={() => setSortAscending((prev: boolean) => !prev)}
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
              {filteredApps.map((app: any, idx: number) => {
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
    </Box>
  );
}
