import dayjs, { Dayjs } from "dayjs";

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { AppUsageCard } from "../components/AppUsageInfo/AppUsageInfo";
import { Box, CircularProgress, Grid, Paper, Typography } from "@mui/material";
import { DatePicker, LocalizationProvider } from "@mui/x-date-pickers";
import { AdapterDayjs } from "@mui/x-date-pickers/AdapterDayjs";

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

export interface IAppUsageInfo {
  appName: string;
  appPath: string;
  totalHours: number;
  idleHours: number;
  activePercentage: number;
  timeLimit: number | null;
  shouldAlert: boolean | null;
  shouldClose: boolean | null;
  alertBeforeClose: boolean | null;
  alertDuration: number | null;
}

export default function AppUsageTab() {
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

  useEffect(() => {
    getAppUsageDetails();
  }, [startDate, endDate]);
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
          appPath: item.appPath || item.app_path,
          totalHours: item.totalHours || item.total_hours || 0,
          idleHours: item.idleHours || item.idle_hours || 0,
          activePercentage:
            item.activePercentage ?? item.active_percentage ?? 0,
          timeLimit: item.timeLimit ?? item.time_limit ?? null,
          shouldAlert: item.shouldAlert ?? item.should_alert ?? null,
          shouldClose: item.shouldClose ?? item.should_close ?? null,
          alertBeforeClose:
            item.alertBeforeClose ?? item.alert_before_close ?? null,
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

  return (
    <LocalizationProvider dateAdapter={AdapterDayjs}>
      <Box
        sx={{ py: 4, px: { xs: 1, sm: 2, md: 3 }, maxWidth: "lg", mx: "auto" }}
      >
        <Typography variant="h4" align="center" gutterBottom>
          App Usage
        </Typography>

        <Box
          sx={{
            mb: 4,
            p: 3,
            borderRadius: 2,
            background: "#ffffff",
            boxShadow: 3,
          }}
        >
          <Typography variant="h6" gutterBottom>
            Select Date Range
          </Typography>
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
          </Box>
        )}
        {fetchError && (
          <Box sx={{ display: "flex", justifyContent: "center", my: 4 }}>
            <Typography color="error">{fetchError}</Typography>
          </Box>
        )}
        {!loading &&
          !fetchError &&
          (!appUsageInfo || appUsageInfo.length === 0) && (
            <Box sx={{ display: "flex", justifyContent: "center", my: 4 }}>
              <Typography>
                No app usage data found for the selected date range.
              </Typography>
            </Box>
          )}
        {appUsageInfo && appUsageInfo.length > 0 && (
          <Grid container spacing={3} direction="column">
            {appUsageInfo.map((val: any, idx: number) => (
              <Grid key={idx}>
                <Paper elevation={3} sx={{ p: 2, borderRadius: 4 }}>
                  <AppUsageCard {...val} />
                </Paper>
              </Grid>
            ))}
          </Grid>
        )}
      </Box>
    </LocalizationProvider>
  );
}
