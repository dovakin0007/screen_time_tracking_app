import React, { useEffect, useState } from "react";
import { IAppUsageInfo } from "../App";
import { invoke } from "@tauri-apps/api/core";

// Import Material UI components
import Card from "@mui/material/Card";
import CardHeader from "@mui/material/CardHeader";
import CardContent from "@mui/material/CardContent";
import Collapse from "@mui/material/Collapse";
import IconButton from "@mui/material/IconButton";
import Typography from "@mui/material/Typography";
import Grid from "@mui/material/Grid";
import TextField from "@mui/material/TextField";
import FormControlLabel from "@mui/material/FormControlLabel";
import Checkbox from "@mui/material/Checkbox";
import Slider from "@mui/material/Slider";
import Box from "@mui/material/Box";
import Avatar from "@mui/material/Avatar";
import Paper from "@mui/material/Paper";
import Chip from "@mui/material/Chip";

// Import Material UI Icons
import ExpandMoreIcon from "@mui/icons-material/ExpandMore";
import ExpandLessIcon from "@mui/icons-material/ExpandLess";

// Keep the HoursAndMinutes class as it's core logic
class HoursAndMinutes {
  hours: number;
  minutes: number;

  constructor(totalMins?: number | null) {
    totalMins = totalMins ?? 0;
    this.hours = Math.floor(totalMins / 60);
    this.minutes = totalMins % 60;
  }

  validateTime() {
    // Keep the 15-minute minimum validation
    return this.hours * 60 + this.minutes >= 15;
  }
}

function AppUsageInfo(props: IAppUsageInfo) {
  const [timeLimit, setTimeLimit] = useState<HoursAndMinutes>(
    new HoursAndMinutes(props.timeLimit),
  );
  const [shouldAlert, setShouldAlert] = useState<boolean>(
    props.shouldAlert ?? false,
  );
  const [shouldClose, setShouldClose] = useState<boolean>(
    props.shouldClose ?? false,
  );
  const [alertBeforeClose, setAlertBeforeClose] = useState<boolean>(
    props.alertBeforeClose ?? false,
  );
  const [alertDuration, setAlertDuration] = useState<number>(
    props.alertDuration ?? 300,
  ); // Default 300s (5 mins)
  const [error, setError] = useState<string>("");
  const [expanded, setExpanded] = useState<boolean>(false);

  // Helper to clamp numbers
  const clamp = (value: number, min: number, max: number) =>
    Math.min(Math.max(value, min), max);

  // Effects for mutual exclusivity between Alert and Close
  useEffect(() => {
    if (shouldAlert && shouldClose) {
      // If both are checked, default to alert and uncheck close
      setShouldClose(false);
    }
  }, [shouldAlert]);

  useEffect(() => {
    if (shouldClose && shouldAlert) {
      // If both are checked, default to close and uncheck alert
      setShouldAlert(false);
    }
  }, [shouldClose]);

  const handleHoursChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseInt(e.target.value, 10);
    // Allow empty input temporarily for user typing
    const hours = isNaN(value) ? 0 : clamp(value, 0, 24);
    const updatedTimeLimit = new HoursAndMinutes(
      hours * 60 + timeLimit.minutes,
    );
    setTimeLimit(updatedTimeLimit);
    setError(
      updatedTimeLimit.validateTime()
        ? ""
        : "Time limit must be at least 15 minutes",
    );
  };

  const handleMinutesChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseInt(e.target.value, 10);
    // Allow empty input temporarily for user typing
    const minutes = isNaN(value) ? 0 : clamp(value, 0, 59);
    const updatedTimeLimit = new HoursAndMinutes(
      timeLimit.hours * 60 + minutes,
    );
    setTimeLimit(updatedTimeLimit);
    setError(
      updatedTimeLimit.validateTime()
        ? ""
        : "Time limit must be at least 15 minutes",
    );
  };

  const handleAlertDurationChange = (
    _event: Event,
    newValue: number | number[],
  ) => {
    setAlertDuration(newValue as number);
  };

  // Effect to trigger Tauri command when settings change and are valid
  useEffect(() => {
    if (timeLimit.validateTime()) {
      triggerDailyLimitChange();
    }
  }, [timeLimit, shouldAlert, shouldClose, alertBeforeClose, alertDuration]);

  const triggerDailyLimitChange = async () => {
    console.log("failed");
    if (!timeLimit.validateTime() && (shouldAlert || shouldClose)) {
      console.warn("Attempted to save settings with an invalid time limit.");
      return;
    }

    try {
      await invoke("set_daily_limit", {
        appName: props.appName,
        totalMinutes: timeLimit.hours * 60 + timeLimit.minutes,
        shouldAlert,
        shouldClose,
        alertBeforeClose,
        alertDuration,
      });
      // Clear error after successful save if it existed before
      if (error && timeLimit.validateTime()) {
        setError("");
      }
    } catch (error) {
      console.error(`Failed to set daily limit for ${props.appName}:`, error);
      // Set a specific error for the save failure if needed
      // setError("Failed to save settings.");
    }
  };

  // Determine if any limit settings are active
  const hasActiveSettings = shouldAlert || shouldClose ||
    (timeLimit.hours * 60 + timeLimit.minutes >= 15); // Check if valid time limit is set

  // Helper function to generate a consistent color for the app icon placeholder
  const getAppIconColor = (appName: string) => {
    const colors = [
      "#4F46E5",
      "#0EA5E9",
      "#10B981",
      "#F59E0B",
      "#EF4444",
      "#8B5CF6",
    ]; // Tailwind color examples
    let hash = 0;
    for (let i = 0; i < appName.length; i++) {
      hash = appName.charCodeAt(i) + ((hash << 5) - hash);
    }
    return colors[Math.abs(hash) % colors.length];
  };

  return (
    <Card
      sx={{
        borderRadius: "16px", // Increased border radius for a softer look
        boxShadow: 3, // Medium shadow
        transition: "box-shadow 0.3s ease-in-out",
        "&:hover": { boxShadow: 6 }, // Larger shadow on hover
        marginBottom: 3, // Space between cards
        border: "1px solid #e0e0e0", // Subtle border
        bgcolor: "background.paper", // Use theme background color
      }}
    >
      <CardHeader
        avatar={
          <Avatar
            sx={{
              bgcolor: getAppIconColor(props.appName),
              width: 40,
              height: 40,
              fontSize: "1rem",
              fontWeight: "medium",
            }}
            aria-label={`${props.appName} icon`}
          >
            {props.appName.substring(0, 1).toUpperCase()}
          </Avatar>
        }
        action={
          <Box sx={{ display: "flex", alignItems: "center" }}>
            {/* Limit Set Chip */}
            {hasActiveSettings && (
              <Chip
                label="LIMIT SET"
                color="success" // Green color
                size="small"
                sx={{ marginRight: 1 }}
              />
            )}
            {/* Expand Button */}
            <IconButton
              onClick={() => setExpanded(!expanded)}
              aria-expanded={expanded}
              aria-label="show more"
              sx={{
                transform: expanded ? "rotate(180deg)" : "rotate(0deg)",
                transition: "transform 0.3s ease-in-out",
              }}
            >
              {expanded ? <ExpandLessIcon /> : <ExpandMoreIcon />}
            </IconButton>
          </Box>
        }
        title={
          <Typography
            variant="h6"
            component="div"
            sx={{ fontWeight: "semibold" }}
          >
            {props.appName}
          </Typography>
        }
        subheader={
          <Typography variant="body2" color="text.secondary">
            {props.totalHours} hours total • {props.activePercentage}% active
          </Typography>
        }
        sx={{
          borderBottom: "1px solid #f0f0f0",
          bgcolor: "#f9f9f9", // Light background for header
        }}
      />
      <Collapse in={expanded} timeout="auto" unmountOnExit>
        <CardContent sx={{ bgcolor: "background.default" }}>
          {/* Slightly different background for content */}
          {/* Stats Section */}
          <Grid container spacing={2} sx={{ marginBottom: 3 }}>
            <Grid size={{ xs: 12, sm: 4 }}>
              {/* Full width on small screens, 1/3 on sm+ */}
              <Paper
                elevation={0}
                sx={{ p: 2, border: "1px solid #e0e0e0", borderRadius: "8px" }}
              >
                <Typography
                  variant="caption"
                  color="text.secondary"
                  display="block"
                  gutterBottom
                >
                  Total Usage
                </Typography>
                <Typography variant="subtitle1" fontWeight="medium">
                  {props.totalHours}h
                </Typography>
              </Paper>
            </Grid>
            <Grid size={{ xs: 12, sm: 4 }}>
              {/* Full width on small screens, 1/3 on sm+ */}
              <Paper
                elevation={0}
                sx={{ p: 2, border: "1px solid #e0e0e0", borderRadius: "8px" }}
              >
                <Typography
                  variant="caption"
                  color="text.secondary"
                  display="block"
                  gutterBottom
                >
                  Active Usage
                </Typography>
                <Typography variant="subtitle1" fontWeight="medium">
                  {(props.totalHours * props.activePercentage / 100).toFixed(
                    1,
                  )}h
                </Typography>
              </Paper>
            </Grid>
            <Grid size={{ xs: 12, sm: 4 }}>
              {/* Full width on small screens, 1/3 on sm+ */}
              <Paper
                elevation={0}
                sx={{ p: 2, border: "1px solid #e0e0e0", borderRadius: "8px" }}
              >
                <Typography
                  variant="caption"
                  color="text.secondary"
                  display="block"
                  gutterBottom
                >
                  Idle Time
                </Typography>
                <Typography variant="subtitle1" fontWeight="medium">
                  {props.idleHours}h
                </Typography>
              </Paper>
            </Grid>
          </Grid>

          {/* Time Limit Section */}
          <Box
            sx={{
              p: 2,
              border: "1px solid #e0e0e0",
              borderRadius: "12px",
              bgcolor: "background.paper",
              marginBottom: 3,
            }}
          >
            <Typography variant="subtitle2" gutterBottom fontWeight="medium">
              Daily Time Limit
            </Typography>
            <Grid container spacing={2}>
              <Grid size={{ xs: 6 }}>
                <TextField
                  label="Hours"
                  type="number"
                  value={timeLimit.hours}
                  onChange={handleHoursChange}
                  slotProps={{
                    htmlInput: {
                      type: "number",
                      min: 0,
                      max: 24,
                    },
                  }}
                  fullWidth
                  size="small"
                  variant="outlined"
                />
              </Grid>
              <Grid size={{ xs: 6 }}>
                <TextField
                  label="Minutes"
                  type="number"
                  value={timeLimit.minutes}
                  onChange={handleMinutesChange}
                  inputProps={{ min: 0, max: 59 }}
                  fullWidth
                  size="small"
                  variant="outlined"
                />
              </Grid>
            </Grid>
            {error && (
              <Typography
                variant="caption"
                color="error"
                sx={{ marginTop: 1, display: "block" }}
              >
                {error}
              </Typography>
            )}
          </Box>

          {/* Alert & Close Settings */}
          <Box sx={{ display: "flex", flexDirection: "column", gap: 1 }}>
            <FormControlLabel
              control={
                <Checkbox
                  checked={shouldAlert}
                  onChange={(e) => setShouldAlert(e.target.checked)}
                  disabled={shouldClose} // Disable if 'Close' is checked
                  size="small"
                />
              }
              label={
                <Typography variant="body2">
                  Alert when limit is reached
                </Typography>
              }
            />

            {shouldAlert && (
              <Box sx={{ ml: 3, mb: 2 }}>
                {/* Indent alert duration setting */}
                <Typography
                  variant="caption"
                  color="text.secondary"
                  display="block"
                  gutterBottom
                >
                  Alert Duration (seconds)
                </Typography>
                <Box sx={{ display: "flex", alignItems: "center", gap: 2 }}>
                  <Slider
                    value={alertDuration}
                    onChange={handleAlertDurationChange}
                    min={10}
                    max={600}
                    step={10} // Slider increments of 10 seconds
                    valueLabelDisplay="auto" // Shows value on thumb hover
                    aria-labelledby="alert-duration-slider"
                    sx={{ width: "calc(100% - 40px)" }} // Make slider take most of the width
                  />
                  <Typography
                    variant="body2"
                    sx={{ minWidth: 40, textAlign: "right" }}
                  >
                    {alertDuration}s
                  </Typography>
                </Box>
              </Box>
            )}

            <FormControlLabel
              control={
                <Checkbox
                  checked={shouldClose}
                  onChange={(e) => setShouldClose(e.target.checked)}
                  disabled={shouldAlert} // Disable if 'Alert' is checked
                  size="small"
                />
              }
              label={
                <Typography variant="body2">
                  Close app when limit is reached
                </Typography>
              }
            />

            {shouldClose && (
              <Box sx={{ ml: 3 }}>
                {/* Indent alert before close setting */}
                <FormControlLabel
                  control={
                    <Checkbox
                      checked={alertBeforeClose}
                      onChange={(e) => setAlertBeforeClose(e.target.checked)}
                      size="small"
                    />
                  }
                  label={
                    <Typography variant="body2">
                      Alert before closing
                    </Typography>
                  }
                />
              </Box>
            )}
          </Box>
        </CardContent>
      </Collapse>
    </Card>
  );
}

export default AppUsageInfo;
