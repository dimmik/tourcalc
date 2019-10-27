import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'


import { BrowserRouter as Router, Route, Switch, Redirect, withRouter } from 'react-router-dom';


export default class ChooseTourVersion extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            tours: null,
            isToursLoaded: false,
            tour: null
        }
    }
    componentWillReceiveProps(props) {
        //alert('rprops')
        this.setState({ isToursLoaded: false, tour: props.tour });
        AppState.loadTourVersions(this, this.props.tour.id, 0, 2000);
    }
    componentDidMount() {
        //alert('ddd')
        AppState.loadTourVersions(this, this.props.tour.id, 0, 2000);
    }
    render() {
        if (!this.props.tour.isVersion) {
            if (!this.state.isToursLoaded) return <span>Loading versions</span>
            return (
                <span><select defaultValue="none"
                    onChange={(e) => { window.location.href = e.target.value }}
                > <option key="none" value="none">Versions</option>
                    {
                        this.state.tours.tours.map((t) => {
                            return <option value={'/tour/' + t.id + '/persons'} key={t.id}>{t.name}</option>
                        }
                        )
                    }
                </select>
                </span>
            )
        } else {
            return <a href={'/tour/' + this.props.tour.versionFor_Id + '/persons'}>Back to current</a>
        }
    }
}
