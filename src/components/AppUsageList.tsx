import { useState } from "react";
import { IAppUsageInfo } from "../App";
import { invoke } from "@tauri-apps/api/core";

function AppUsageInfo(props: IAppUsageInfo) {
  const [hours, setHours] = useState<number>(0);
  const [minutes, setMinutes] = useState<number>(0);
  const [error, setError] = useState<string>("");

  const clamp = (value: number, min: number, max: number) => {
    return Math.min(Math.max(value, min), max);
  };

  const validateTime = (hrs: number, mins: number) => {
    const totalMinutes = hrs * 60 + mins;

    if (totalMinutes <= 15) {
      setError("Daily limit must be more than 15 minutes.");
      return false;
    } else {
      setError("");
      return true;
    }
  };

  const handleHoursChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const rawValue = parseInt(e.target.value, 10);
    const clampedHours = clamp(isNaN(rawValue) ? 0 : rawValue, 0, 24);
    setHours(clampedHours);

    if (validateTime(clampedHours, minutes)) {
      triggerDailyLimitChange(clampedHours, minutes);
    }
  };

  const handleMinutesChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const rawValue = parseInt(e.target.value, 10);
    const clampedMinutes = clamp(isNaN(rawValue) ? 0 : rawValue, 0, 59);
    setMinutes(clampedMinutes);

    if (validateTime(hours, clampedMinutes)) {
      triggerDailyLimitChange(hours, clampedMinutes);
    }
  };

  const triggerDailyLimitChange = async (hrs: number, mins: number) => {
    const totalMinutes = hrs * 60 + mins;
    console.log(props.appName);
    try {
      const response = await invoke("set_daily_limit", {
        appName: props.appName,
        totalMinutes: totalMinutes,
      });
      console.log(`Daily limit set for ${props.appName}:`, response);
    } catch (error) {
      console.error(`Failed to set daily limit for ${props.appName}:`, error);
    }
  };

  return (
    <div className="bg-white shadow-lg rounded-2xl p-6 w-80 border border-gray-200">
      <h2 className="text-xl font-semibold text-gray-800 mb-4">App Usage Info</h2>
      <div className="space-y-2">
        <p className="text-gray-600">
          <strong className="text-gray-800">App Name:</strong> {props.appName}
        </p>
        <p className="text-gray-600">
          <strong className="text-gray-800">Total Hours:</strong> {props.totalHours}
        </p>
        <p className="text-gray-600">
          <strong className="text-gray-800">Idle Hours:</strong> {props.idleHours}
        </p>
        <p className="text-gray-600">
          <strong className="text-gray-800">Active Percentage:</strong> {props.activePercentage}%
        </p>

        <div className="text-gray-600 mt-4">
          <label className="block text-gray-800 font-semibold mb-1">
            Daily Limit:
          </label>

          <div className="flex space-x-4">
            <div className="flex-1">
              <input
                type="number"
                min={0}
                max={24}
                value={hours}
                onChange={handleHoursChange}
                className="w-full border border-gray-300 rounded px-3 py-2 focus:outline-none focus:ring-2 focus:ring-blue-500"
                placeholder="Hours"
              />
            </div>
            <div className="flex-1">
              <input
                type="number"
                min={0}
                max={59}
                value={minutes}
                onChange={handleMinutesChange}
                className="w-full border border-gray-300 rounded px-3 py-2 focus:outline-none focus:ring-2 focus:ring-blue-500"
                placeholder="Minutes"
              />
            </div>
          </div>

          {error && (
            <p className="text-red-500 text-sm mt-2">{error}</p>
          )}
        </div>
      </div>
    </div>
  );
}

export default AppUsageInfo;


