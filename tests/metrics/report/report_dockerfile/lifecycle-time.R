#!/usr/bin/env Rscript
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Display how long the various phases of a container lifecycle (run, execute, die etc.
# take.

library(ggplot2)					# ability to plot nicely.
suppressMessages(suppressWarnings(library(tidyr)))	# for gather().
							# So we can plot multiple graphs
library(gridExtra)					# together.
suppressMessages(suppressWarnings(library(ggpubr)))	# for ggtexttable.
suppressMessages(library(jsonlite))			# to load the data.

testnames=c(
	"boot-times"
)

data=c()
stats=c()
rstats=c()
rstats_names=c()

# For each set of results
for (currentdir in resultdirs) {
	count=1
	dirstats=c()
	for (testname in testnames) {
		fname=paste(inputdir, currentdir, testname, '.json', sep="")
		if ( !file.exists(fname)) {
			warning(paste("Skipping non-existent file: ", fname))
			next
		}

		# Derive the name from the test result dirname
		datasetname=basename(currentdir)

		# Import the data
		fdata=fromJSON(fname)
		# De-nest the test specific name
		fdata=fdata[[testname]]

		cdata=data.frame(workload=as.numeric(fdata$Results$'to-workload'$Result))
		cdata=cbind(cdata, quit=as.numeric(fdata$Results$'to-quit'$Result))

		cdata=cbind(cdata, tokernel=as.numeric(fdata$Results$'to-kernel'$Result))
		cdata=cbind(cdata, inkernel=as.numeric(fdata$Results$'in-kernel'$Result))
		cdata=cbind(cdata, total=as.numeric(fdata$Results$'total'$Result))

		cdata=cbind(cdata, count=seq_len(length(cdata[,"workload"])))
		cdata=cbind(cdata, runtime=rep(datasetname, length(cdata[, "workload"]) ))

		# Calculate some stats for total time
		sdata=data.frame(workload_mean=mean(cdata$workload))
		sdata=cbind(sdata, workload_min=min(cdata$workload))
		sdata=cbind(sdata, workload_max=max(cdata$workload))
		sdata=cbind(sdata, workload_sd=sd(cdata$workload))
		sdata=cbind(sdata, workload_cov=((sdata$workload_sd / sdata$workload_mean) * 100))
		sdata=cbind(sdata, runtime=datasetname)

		sdata=cbind(sdata, quit_mean = mean(cdata$quit))
		sdata=cbind(sdata, quit_min = min(cdata$quit))
		sdata=cbind(sdata, quit_max = max(cdata$quit))
		sdata=cbind(sdata, quit_sd = sd(cdata$quit))
		sdata=cbind(sdata, quit_cov = (sdata$quit_sd / sdata$quit_mean) * 100)

		sdata=cbind(sdata, tokernel_mean = mean(cdata$tokernel))
		sdata=cbind(sdata, inkernel_mean = mean(cdata$inkernel))
		sdata=cbind(sdata, total_mean = mean(cdata$total))

		# Store away as a single set
		data=rbind(data, cdata)
		stats=rbind(stats, sdata)

		# Store away some stats for the text table
		dirstats[count]=round(sdata$tokernel_mean, digits=2)
		count = count + 1
		dirstats[count]=round(sdata$inkernel_mean, digits=2)
		count = count + 1
		dirstats[count]=round(sdata$workload_mean, digits=2)
		count = count + 1
		dirstats[count]=round(sdata$quit_mean, digits=2)
		count = count + 1
		dirstats[count]=round(sdata$total_mean, digits=2)
		count = count + 1
	}
	rstats=rbind(rstats, dirstats)
	rstats_names=rbind(rstats_names, datasetname)
}

unts=c("s", "s", "s", "s", "s")
rstats=rbind(rstats, unts)
rstats_names=rbind(rstats_names, "Units")


# If we have only 2 sets of results, then we can do some more
# stats math for the text table
if (length(resultdirs) == 2) {
	# This is a touch hard wired - but we *know* we only have two
	# datasets...
	diff=c()
	for( i in 1:5) {
		difference = as.double(rstats[2,i]) - as.double(rstats[1,i])
		val = 100 * (difference/as.double(rstats[1,i]))
		diff[i] = paste(round(val, digits=2), "%", sep=" ")
	}

	rstats=rbind(rstats, diff)
	rstats_names=rbind(rstats_names, "Diff")
}

rstats=cbind(rstats_names, rstats)

# Set up the text table headers
colnames(rstats)=c("Results", "2k", "ik", "2w", "2q", "tot")


# Build us a text table of numerical results
stats_plot = suppressWarnings(ggtexttable(data.frame(rstats, check.names=FALSE),
	theme=ttheme(base_size=8),
	rows=NULL
	))

# plot how samples varioed over  'time'
line_plot <- ggplot() +
	geom_line( data=data, aes(count, workload, color=runtime)) +
	geom_smooth( data=data, aes(count, workload, color=runtime), se=FALSE, method="loess") +
	xlab("Iteration") +
	ylab("Time (s)") +
	ggtitle("Boot to workload", subtitle="First container") +
	ylim(0, NA) +
	theme(axis.text.x=element_text(angle=90))

boot_boxplot <- ggplot() +
	geom_boxplot( data=data, aes(runtime, workload, color=runtime), show.legend=FALSE) +
	ylim(0, NA) +
	ylab("Time (s)")

# convert the stats to a long format so we can more easily do a side-by-side barplot
longstats <- gather(stats, measure, value, workload_mean, quit_mean, inkernel_mean, tokernel_mean, total_mean)

bar_plot <- ggplot() +
	geom_bar( data=longstats, aes(measure, value, fill=runtime), stat="identity", position="dodge", show.legend=FALSE) +
	xlab("Phase") +
	ylab("Time (s)") +
	ggtitle("Lifecycle phase times", subtitle="Mean") +
	ylim(0, NA) +
	theme(axis.text.x=element_text(angle=90))

master_plot = grid.arrange(
	bar_plot,
	line_plot,
	stats_plot,
	boot_boxplot,
	nrow=2,
	ncol=2 )

