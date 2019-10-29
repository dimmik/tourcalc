import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import ChooseTourVersion from './tour-versions.jsx'

import { BrowserRouter as Router, Route, Switch, Redirect, withRouter } from 'react-router-dom';


export default class TourInfo extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            tour: props.tour,
            updateTime: props.updateTime,
            expanded: false
        }
    }
    componentWillReceiveProps(props) {
        this.setState({ tour: props.tour, updateTime: props.updateTime });
    }
    componentDidMount() {
    }
    render() {

        return (
            <div style={{ fontSize: 'small' }}>
                <span
                    style={{ cursor: 'pointer', textDecoration: 'underline', borderStyle: 'ridge' }}
                    onClick={
                        () => { AppState.loadTour(this.props.app, this.props.tourid); }
                    }>Refresh</span>
                &nbsp;
                            {this.state.tour.isVersion ? <b>(ver {this.state.tour.dateVersioned}: before {this.state.tour.versionComment})&nbsp;</b> : ''}

                {this.state.tour.name} [
                                {
                    this.state.tour.persons.filter(p => (p.receivedInCents - p.spentInCents) >= 0).length > 0
                        ? ((1 - this.state.tour.persons.filter(p => (p.receivedInCents - p.spentInCents) > 0).length * 1.0 /
                            this.state.tour.persons.filter(p => (p.receivedInCents - p.spentInCents) >= 0).length) * 100)
                            .toFixed(0) : 0
                }%&nbsp;

                        {
                    (this.state.updateTime.getHours() + "").padStart(2, '0') + ':' +
                    (this.state.updateTime.getMinutes() + "").padStart(2, '0') + ':' +
                    (this.state.updateTime.getSeconds() + "").padStart(2, '0')
                }]

               

                            <a href="/">List</a>&nbsp;&nbsp;
                            <ChooseTourVersion tour={this.state.tour} />
            </div>
            )

    }
}
