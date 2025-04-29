import React, { useEffect, useState } from "react";
import { IAppUsageInfo } from "../../pages/AppUsageTab";
import { invoke } from "@tauri-apps/api/core";
import {
  Avatar,
  Box,
  Card,
  CardContent,
  CardHeader,
  Chip,
  Collapse,
  IconButton,
  Typography,
} from "@mui/material";
import {
  ExpandLess as ExpandLessIcon,
  ExpandMore as ExpandMoreIcon,
} from "@mui/icons-material";

import { TimeLimitInput } from "./TimeInput";
import { UsageStats } from "./UsageStats";
import { AlertSettings } from "./AlertSettings";

class HoursAndMinutes {
  hours: number;
  minutes: number;

  constructor(totalMins?: number | null) {
    totalMins = totalMins ?? 0;
    this.hours = Math.floor(totalMins / 60);
    this.minutes = totalMins % 60;
  }

  validateTime() {
    return this.hours * 60 + this.minutes >= 15;
  }
}

export const AppUsageCard: React.FC<IAppUsageInfo> = (props) => {
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
  );
  const [error, setError] = useState<string>("");
  const [expanded, setExpanded] = useState<boolean>(false);
  const [iconBase64, setIconBase64] = useState<string | null>(null);

  useEffect(() => {
    if (shouldAlert && shouldClose) {
      setShouldClose(false);
    }
  }, [shouldAlert]);

  useEffect(() => {
    if (shouldClose && shouldAlert) {
      setShouldAlert(false);
    }
  }, [shouldClose]);

  useEffect(() => {
    async function fetchIcon() {
      try {
        const base64: string = await invoke("fetch_app_icon", {
          path: props.appPath,
        });
        setIconBase64(base64);
      } catch (error) {
        console.error(`Failed to fetch icon for ${props.appPath}:`, error);
      }
    }

    fetchIcon();
  }, [props.appPath]);

  useEffect(() => {
    if (timeLimit.validateTime()) {
      triggerDailyLimitChange();
    }
  }, [timeLimit, shouldAlert, shouldClose, alertBeforeClose, alertDuration]);

  const triggerDailyLimitChange = async () => {
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
      if (error && timeLimit.validateTime()) {
        setError("");
      }
    } catch (error) {
      console.error(`Failed to set daily limit for ${props.appName}:`, error);
    }
  };

  const handleTimeLimitChange = (hours: number, minutes: number) => {
    const updatedTimeLimit = new HoursAndMinutes(hours * 60 + minutes);
    setTimeLimit(updatedTimeLimit);
    setError(
      updatedTimeLimit.validateTime()
        ? ""
        : "Time limit must be at least 15 minutes",
    );
  };

  const hasActiveSettings = shouldAlert || shouldClose ||
    (timeLimit.hours * 60 + timeLimit.minutes >= 15);

  const getAppIconColor = (appName: string) => {
    const colors = [
      "#4F46E5",
      "#0EA5E9",
      "#10B981",
      "#F59E0B",
      "#EF4444",
      "#8B5CF6",
    ];
    let hash = 0;
    for (let i = 0; i < appName.length; i++) {
      hash = appName.charCodeAt(i) + ((hash << 5) - hash);
    }
    return colors[Math.abs(hash) % colors.length];
  };

  return (
    <Card
      sx={{
        borderRadius: "16px",
        boxShadow: 3,
        transition: "box-shadow 0.3s ease-in-out",
        "&:hover": { boxShadow: 6 },
        marginBottom: 3,
        border: "1px solid #e0e0e0",
        bgcolor: "background.paper",
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
            src={iconBase64 ? `data:image/png;base64,${iconBase64}` : undefined}
          >
            {!iconBase64 && props.appName.substring(0, 1).toUpperCase()}
          </Avatar>
        }
        action={
          <Box sx={{ display: "flex", alignItems: "center" }}>
            {hasActiveSettings && (
              <Chip
                label="LIMIT SET"
                color="success"
                size="small"
                sx={{ marginRight: 1 }}
              />
            )}
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
          bgcolor: "#f9f9f9",
        }}
      />
      <Collapse in={expanded} timeout="auto" unmountOnExit>
        <CardContent sx={{ bgcolor: "background.default" }}>
          <UsageStats
            totalHours={props.totalHours}
            activePercentage={props.activePercentage}
            idleHours={props.idleHours}
          />

          <TimeLimitInput
            hours={timeLimit.hours}
            minutes={timeLimit.minutes}
            onHoursChange={(hours: number) =>
              handleTimeLimitChange(hours, timeLimit.minutes)}
            onMinutesChange={(minutes: number) =>
              handleTimeLimitChange(timeLimit.hours, minutes)}
            error={error}
          />

          <AlertSettings
            shouldAlert={shouldAlert}
            shouldClose={shouldClose}
            alertBeforeClose={alertBeforeClose}
            alertDuration={alertDuration}
            onAlertChange={setShouldAlert}
            onCloseChange={setShouldClose}
            onAlertBeforeCloseChange={setAlertBeforeClose}
            onAlertDurationChange={setAlertDuration}
          />
        </CardContent>
      </Collapse>
    </Card>
  );
};
