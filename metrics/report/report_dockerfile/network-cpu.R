#!/usr/bin/env Rscript
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Analyse the runtime component memory footprint data.

library(ggplot2)					# ability to plot nicely.
							# So we can plot multiple graphs
library(gridExtra)					# together.
suppressMessages(suppressWarnings(library(ggpubr)))	# for ggtexttable.
suppressMessages(library(jsonlite))			# to load the data.

testnames=c(
	"cpu-information"
)

resultsfilesshort=c(
	"CPU"
)

data=c()
rstats=c()
rstats_rows=c()
rstats_cols=c()

Gdenom = (1000.0 * 1000.0 * 1000.0)

# For each set of results
for (currentdir in resultdirs) {
	dirstats=c()
	# For the two different types of memory footprint measures
	for (testname in testnames) {
		# R seems not to like double path slashes '//' ?
		fname=paste(inputdir, currentdir, testname, '.json', sep="")
		if ( !file.exists(fname)) {
			warning(paste("Skipping non-existent file: ", fname))
			next
		}

		# Derive the name from the test result dirname
		datasetname=basename(currentdir)
		datasetvariant=resultsfilesshort[count]

		# Import the data
		fdata=fromJSON(fname)
		fdata=fdata[[testname]]
		# Copy the average result into a shorter, more accesible name
		fdata$ips=fdata$Results$"instructions per cycle"$Result
		fdata$Gcycles=fdata$Results$cycles$Result / Gdenom
		fdata$Ginstructions=fdata$Results$instructions$Result / Gdenom
		fdata$variant=rep(datasetvariant, length(fdata$Result) )
		fdata$Runtime=rep(datasetname, length(fdata$Result) )

		# Store away the bits we need
		data=rbind(data, data.frame(
			Result=fdata$ips,
			Type="ips",
			Runtime=fdata$Runtime,
			variant=fdata$variant ) )
		data=rbind(data, data.frame(
			Result=fdata$Gcycles,
			Type="Gcycles",
			Runtime=fdata$Runtime,
			variant=fdata$variant ) )
		data=rbind(data, data.frame(
			Result=fdata$Ginstructions,
			Type="Ginstr",
			Runtime=fdata$Runtime,
			variant=fdata$variant ) )

		# Store away some stats for the text table
		dirstats=rbind(dirstats, round(fdata$ips, digits=2) )
		dirstats=rbind(dirstats, round(fdata$Gcycles, digits=2) )
		dirstats=rbind(dirstats, round(fdata$Ginstructions, digits=2) )
	}
	rstats=cbind(rstats, dirstats)
	rstats_cols=append(rstats_cols, datasetname)
}

rstats_rows=c("IPS", "GCycles", "GInstr")

unts=c("Ins/Cyc", "G", "G")
rstats=cbind(rstats, unts)
rstats_cols=append(rstats_cols, "Units")

# If we have only 2 sets of results, then we can do some more
# stats math for the text table
if (length(resultdirs) == 2) {
	# This is a touch hard wired - but we *know* we only have two
	# datasets...
	diff=c()
	for (n in 1:3) {
		difference = (as.double(rstats[n,2]) - as.double(rstats[n,1]))
		val = 100 * (difference/as.double(rstats[n,1]))
		diff=rbind(diff, round(val, digits=2))
	}
	rstats=cbind(rstats, diff)
	rstats_cols=append(rstats_cols, "Diff %")
}

# Build us a text table of numerical results
stats_plot = suppressWarnings(ggtexttable(data.frame(rstats),
	theme=ttheme(base_size=10),
	rows=rstats_rows, cols=rstats_cols
	))

# plot how samples varioed over  'time'
ipsdata <- subset(data, Type %in% c("ips"))
ips_plot <- ggplot() +
	geom_bar(data=ipsdata, aes(Type, Result, fill=Runtime), stat="identity", position="dodge") +
	xlab("Measure") +
	ylab("IPS") +
	ggtitle("Instructions Per Cycle") +
	ylim(0, NA) +
	theme(axis.text.x=element_text(angle=90))

cycdata <- subset(data, Type %in% c("Gcycles", "Ginstr"))
cycles_plot <- ggplot() +
	geom_bar(data=cycdata, aes(Type, Result, fill=Runtime), stat="identity", position="dodge", show.legend=FALSE) +
	xlab("Measure") +
	ylab("Count (G)") +
	ggtitle("Cycles and Instructions") +
	ylim(0, NA) +
	theme(axis.text.x=element_text(angle=90))

master_plot = grid.arrange(
	ips_plot,
	cycles_plot,
	stats_plot,
	nrow=2,
	ncol=2 )

